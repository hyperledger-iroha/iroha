---
lang: am
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ የምርት ፍልሰት መመሪያ

ይህ Runbook የStage6 ምርት FASTPQ prover እንዴት ማረጋገጥ እንደሚቻል ይገልጻል።
የዚህ የፍልሰት እቅድ አካል ሆኖ የሚወስነው የቦታ ያዥ ጀርባ ተወግዷል።
በ`docs/source/fastpq_plan.md` ውስጥ የታቀደውን እቅድ ያሟላል እና እርስዎ እንደተከታተሉት ይገምታል
የስራ ቦታ ሁኔታ በ `status.md`.

## ታዳሚዎች እና ወሰን
- የማረጋገጫ ኦፕሬተሮች የማምረቻውን ፕሮፌሽናል በማዘጋጀት ወይም በሜይንኔት አከባቢዎች ውስጥ ያሰራጩ።
- ከአምራች ጀርባ ጋር የሚጓጓዙ ሁለትዮሽ ወይም ኮንቴይነሮችን የሚፈጥሩ መሐንዲሶችን ይልቀቁ።
- SRE/ተመልካች ቡድኖች አዲስ የቴሌሜትሪ ምልክቶችን ሽቦ እና ማንቂያ።

ከጥቅም ውጭ፡ Kotodama ውል ደራሲ እና IVM ABI ለውጦች (ለዚህ `docs/source/nexus.md` ይመልከቱ
የማስፈጸሚያ ሞዴል).

## የባህሪ ማትሪክስ
| መንገድ | ለማንቃት ጭነት ባህሪያት | ውጤት | መቼ መጠቀም |
| ---- | --------------------------------- | ------ | ----------- |
| የምርት prover (ነባሪ) | _ምንም_ | ደረጃ 6 FASTPQ ጀርባ ከFFT/LDE እቅድ አውጪ እና ከDEEP-FRI ቧንቧ መስመር ጋር።【crates/fastpq_prover/src/backend.rs:1144】 | ለሁሉም የምርት ሁለትዮሽ ነባሪ። |
| አማራጭ ጂፒዩ ማጣደፍ | `fastpq_prover/fastpq-gpu` | CUDA/Metal kernels ከአውቶማቲክ ሲፒዩ ውድቀት ጋር ያነቃል።【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | የሚደገፉ ፍጥነቶች ያሉት አስተናጋጆች። |

## የግንባታ ሂደት
1. ** ሲፒዩ-ብቻ ግንባታ ***
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   የምርት ጀርባው በነባሪነት ተሰብስቧል; ምንም ተጨማሪ ባህሪያት አያስፈልጉም.

2. **ጂፒዩ የነቃ ግንባታ (አማራጭ)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   የጂፒዩ ድጋፍ በግንባታው ወቅት የሚገኝ `nvcc` ያለው የSM80+ CUDA መሣሪያ ስብስብ ይፈልጋል።【crates/fastpq_prover/Cargo.toml:11】

3. ** ራስን መፈተሽ ***
   ```bash
   cargo test -p fastpq_prover
   ```
   ከመታሸጉ በፊት የStage6 ዱካ ለማረጋገጥ ይህንን በየልቀት ግንባታ አንድ ጊዜ ያሂዱ።

### የብረት መሣሪያ ሰንሰለት ዝግጅት (ማክኦኤስ)
1. ከመገንባቱ በፊት የብረታ ብረት ማዘዣ መሳሪያዎችን ይጫኑ፡- `xcode-select --install` (የ CLI መሳሪያዎች ከጠፉ) እና `xcodebuild -downloadComponent MetalToolchain` የጂፒዩ የመሳሪያ ሰንሰለት ለማምጣት። የግንባታ ስክሪፕቱ `xcrun metal`/`xcrun metallib` በቀጥታ ይጠራል እና ሁለትዮሽዎቹ ከሌሉ በፍጥነት ይከሽፋል።【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121
2. ከCI በፊት ያለውን የቧንቧ መስመር ለማረጋገጥ፣ የግንባታ ስክሪፕቱን በአገር ውስጥ ማንጸባረቅ ይችላሉ፡-
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   ይህ ሲሳካ ግንባታው `FASTPQ_METAL_LIB=<path>` ያወጣል; የሩጫ ሰዓቱ ሜታሊብን በቆራጥነት ለመጫን ያንን እሴት ያነባል።【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. ያለ ብረት የመሳሪያ ሰንሰለት ሲሻገር `FASTPQ_SKIP_GPU_BUILD=1` አዘጋጅ; ግንባታው ማስጠንቀቂያ ያትማል እና እቅድ አውጪው በሲፒዩ ዱካ ላይ ይቆያል።
4. Metal የማይገኝ ከሆነ (የጠፋ ማዕቀፍ፣ የማይደገፍ ጂፒዩ፣ ወይም ባዶ `FASTPQ_METAL_LIB`) ከሆነ አንጓዎች በራስ ሰር ወደ ሲፒዩ ይመለሳሉ። የግንባታው ስክሪፕት env varን ያጸዳል እና እቅድ አውጪው ዝቅተኛ ደረጃውን ይመዘግባል።### የመልቀቂያ ዝርዝር (ደረጃ 6)
ከታች ያለው እያንዳንዱ ንጥል ነገር እስኪጠናቀቅ እና እስኪያያዘ ድረስ የFASTPQ የመልቀቂያ ትኬት እንደታገደ ያቆዩት።

1. ** የንዑስ ሰከንድ ማረጋገጫ መለኪያዎች *** - አዲስ የተያዙትን `fastpq_metal_bench_*.json` ይፈትሹ እና
   `benchmarks.operations` መግቢያውን ያረጋግጡ `operation = "lde"` (እና የተንጸባረቀው)
   `report.operations` ናሙና) `gpu_mean_ms ≤ 950` ለ 20000 ረድፎች የሥራ ጫና (32768 የታሸገ) ሪፖርት አድርጓል
   ረድፎች). የማረጋገጫ ዝርዝሩ ከመፈረሙ በፊት ከጣሪያው ውጭ ያሉ ቀረጻዎች እንደገና መካሄድ አለባቸው።
2. ** የተፈረመ አንጸባራቂ ** - ሩጫ
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   ስለዚህ የመልቀቂያ ትኬቱ ሁለቱንም አንጸባራቂ እና የተለየ ፊርማ ይይዛል
   (`artifacts/fastpq_bench_manifest.sig`)። ገምጋሚዎች የምግብ መፍጫውን/ፊርማውን ጥንድ ከዚህ በፊት ያረጋግጣሉ
   መልቀቅን ማስተዋወቅ።【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 የማትሪክስ አንጸባራቂ (የተሰራ
   በ`scripts/fastpq/capture_matrix.sh`) ቀድሞውንም 20k የረድፍ ወለል እና
   ሪግሬሽን ማረም.
3. **የማስረጃ አባሪዎች** — የብረታ ብረት መለኪያ JSON፣ stdout log (ወይም Instruments trace) ይስቀሉ፣
   CUDA/Metal አንጸባራቂ ውጤቶች፣ እና የተለቀቀው ቲኬት ፊርማ። የማረጋገጫ ዝርዝሩ ግቤት
   ወደ ታች የተፋሰሱ ኦዲቶችን ለመፈረም ጥቅም ላይ ከሚውለው የወል ቁልፍ አሻራ ጋር ከሁሉም ቅርሶች ጋር ማገናኘት አለበት።
   የማረጋገጫ ደረጃውን እንደገና ማጫወት ይችላል።【አርቲፊክስ/fastpq_benchmarks/README.md:65】### የብረታ ብረት ማረጋገጫ የስራ ፍሰት
1. በጂፒዩ ከነቃ ግንባታ በኋላ የ`FASTPQ_METAL_LIB` ነጥቦችን በ`.metallib` (`echo $FASTPQ_METAL_LIB`) ያረጋግጡ ስለዚህ የሩጫ ጊዜው በትክክል ሊጭነው ይችላል።【crates/fastpq_prover/build.rs:188】
2. የተመጣጣኝ ስብስብን በጂፒዩ መስመሮች በግዳጅ ያሂዱ፡\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. የጀርባው አካል የብረታ ብረት ከርነሎችን ይጠቀማል እና ማወቂያው ካልተሳካ የሚወስን የሲፒዩ ውድቀት ይመዘግባል።
3. ለዳሽቦርዶች የቤንችማርክ ናሙና ያንሱ፡\
   የተሰበሰበው የብረታ ብረት ላይብረሪ (`fd -g 'fastpq.metallib' target/release/build | head -n1`) ያግኙ፣
   በ`FASTPQ_METAL_LIB` በኩል ይላኩት እና ያሂዱ።
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  ቀኖናዊው `fastpq-lane-balanced` መገለጫ አሁን እያንዳንዱን ቀረጻ ወደ 32,768 ረድፎች (2¹⁵) ይሸፍነዋል፣ ስለዚህ JSON ሁለቱንም `rows` እና `padded_rows` ከብረት LDE መዘግየት ጋር ይይዛል። `zero_fill` ወይም የወረፋ ቅንጅቶች የጂፒዩ LDEን ከ950ms (<1s) ኢላማ በ AppleM-series hosts ከገፋው ቀረጻውን እንደገና ያስጀምሩት። የተገኘውን JSON/ሎግ ከሌሎች የመልቀቂያ ማስረጃዎች ጋር በማህደር ያስቀምጡ፤ የምሽት የማክኦኤስ የስራ ፍሰት ተመሳሳይ ሩጫን ያከናውናል እና ቅርሶቹን ለማነፃፀር ይሰቀላል።
  የፖሲዶን ብቻ ቴሌሜትሪ ሲፈልጉ (ለምሳሌ፣ የመሣሪያዎች ዱካ ለመቅዳት) ከላይ ባለው ትዕዛዝ ላይ `--operation poseidon_hash_columns` ይጨምሩ። አግዳሚ ወንበሩ አሁንም `FASTPQ_GPU=gpu`ን ያከብራል፣ `metal_dispatch_queue.poseidon` ያወጣል እና አዲሱን `poseidon_profiles` ብሎክን ይጨምራል ስለዚህ የተለቀቀው ጥቅል የፖሲዶን ማነቆን በግልፅ ያሳያል።
  ማስረጃው አሁን `zero_fill.{bytes,ms,queue_delta}` እና `kernel_profiles` (በከርነል) ያካትታል
  የመኖሪያ ቦታ፣ የሚገመተው GB/s፣ እና የቆይታ ጊዜ ስታቲስቲክስ) ስለዚህ የጂፒዩ ቅልጥፍና ያለሱ ሊቀረጽ ይችላል።
  ጥሬ ዱካዎችን እንደገና በማዘጋጀት ላይ፣ እና `twiddle_cache` ብሎክ (መምታት/ያመለጡ + `before_ms`/`after_ms`)
  የተሸጎጡ ትዊድል ሰቀላዎች በስራ ላይ መሆናቸውን ያረጋግጣል። `--trace-dir` መታጠቂያ ስር ዳግም ያስጀምራል
  `xcrun xctrace record` እና
  በጊዜ ማህተም የተደረገ `.trace` ፋይል ከJSON ጋር ያከማቻል; አሁንም ሹመት መስጠት ይችላሉ።
  ወደ
  ብጁ አካባቢ / አብነት. JSON ለኦዲት `metal_trace_{template,seconds,output}` ይመዘግባል።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】ከእያንዳንዱ ቀረጻ በኋላ `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` ን ያሂዱ ስለዚህ ህትመቱ አስተናጋጅ ሜታዳታ (አሁን `metadata.metal_trace`) ለGrafana ቦርድ/የማስጠንቀቂያ ቅርቅብ (`dashboards/grafana/fastpq_acceleration.json`፣ `dashboards/alerts/fastpq_acceleration_rules.yml`) ይይዛል። ሪፖርቱ አሁን `speedup` ነገር በአንድ ኦፕሬሽን (`speedup.ratio`፣ `speedup.delta_ms`)፣ መጠቅለያው `zero_fill_hotspots` (ባይት፣ መዘግየት፣ የተገኘ GB/s፣ እና የብረታ ብረት ወረፋ ዴልታ100701010)፣ `benchmarks.kernel_summary`፣ የ`twiddle_cache` ብሎክን እንደጠበቀ ያቆያል፣ አዲሱን `post_tile_dispatches` ብሎክ/ማጠቃለያ ገልብጦ ገምጋሚዎች በቀረጻው ወቅት የሰራውን ባለብዙ ማለፊያ ከርነል እንዲያረጋግጡ እና አሁን የፖሲዶን ማይክሮ ቤንች ማስረጃን ወደ Prometheus ገልጿል። ጥሬውን ሪፖርቱን ሳይመልስ scalar-vs-default መዘግየት። አንጸባራቂው በር ያንኑ ብሎክ ያነባል እና እሱን የሚተዉትን የጂፒዩ ማስረጃዎች ውድቅ ያደርጋል፣ ይህም ኦፕሬተሮች የድህረ ንጣፍ መንገድ በተዘለለ ቁጥር ምስሎችን እንዲያድስ ያስገድዳቸዋል። በተሳሳተ መንገድ የተዋቀረ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【ስክሪፕቶች/fastpq /wrap_benchmark.py:714】【ስክሪፕቶች/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  የPoseidon2 Metal kernel ተመሳሳይ ቁልፎችን ይጋራል፡- `FASTPQ_METAL_POSEIDON_LANES` (32-256፣ የሁለት ሃይሎች) እና `FASTPQ_METAL_POSEIDON_BATCH` (1-32 ግዛቶች በአንድ ሌይን) የማስጀመሪያውን ስፋት እና የሌይን ስራ እንደገና ሳይገነቡ እንዲሰኩ ያስችልዎታል። አስተናጋጁ እነዚያን እሴቶች ከእያንዳንዱ መላኪያ በፊት በ`PoseidonArgs` በኩል ይከራል። በነባሪነት የሩጫ ሰዓቱ `MTLDevice::{is_low_power,is_headless,location}` ወደ ቪራም-ደረጃ ያላቸው ጅምርዎች አድልዎ ወደሚታይባቸው ጂፒዩዎች ይፈትሻል (≥48GiB ሪፖርት ሲደረግ `256×24`፣ `256×20` በ32GiB፣Prometheus ያለበለዚያ ዝቅተኛ-ኃይሉ ሲቀረው)82X `256×8` (እና የቆዩ 128/64 ሌይን ክፍሎች በአንድ ሌይን ከ8/6 ግዛቶች ጋር ይጣበቃሉ)፣ ስለዚህ አብዛኛዎቹ ኦፕሬተሮች የኢንቪ ቫርስን በእጅ ማቀናበር በፍፁም አያስፈልጋቸውም። `fastpq_metal_bench` አሁን እራሱን በ `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ስር እንደገና ይሰራል እና `poseidon_microbench` ብሎክ ያመነጫል ሁለቱንም የማስጀመሪያ ፕሮፋይሎች እና የሚለካውን ፍጥነት ከስካላር ሌይን ጋር ይመዘግባል ስለዚህ ጥቅሎች አዲሱን ከርነል በትክክል ይቀንሳል Prometheus `poseidon_pipeline` ብሎክ ስለዚህ የStage7 ማስረጃ ከአዲሱ የነዋሪነት እርከኖች ጎን ለጎን የጥልቁን ጥልቀት/መደራረብን ይይዛል። ለመደበኛ ሩጫዎች env እንዳልተዋቀረ ይተዉት; መታጠቂያው ድጋሚ አፈጻጸምን በራስ ሰር ያስተዳድራል፣ ልጁ ቀረጻ መሮጥ ካልቻለ ያልተሳካለትን ምዝግብ ማስታወሻ ይይዛል፣ እና `FASTPQ_GPU=gpu` ሲዘጋጅ ወዲያውኑ ይወጣል ነገር ግን ምንም የጂፒዩ ጀርባ የለም ስለዚህ ጸጥ ያለ የሲፒዩ ውድቀት ወደ ፐርፍ ሾልኮ አይሄድም። artefacts.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】መጠቅለያው የ `metal_dispatch_queue.poseidon` ዴልታ፣ የተጋሩ `column_staging` ቆጣሪዎች፣ ወይም `poseidon_profiles`/`poseidon_microbench` ማስረጃዎችን የጎደለውን የፖሲዶን ቀረጻዎች ውድቅ ያደርጋል፣ ስለዚህ ኦፕሬተሮቹ ያልተሳካውን ወይም ያልተሳካለትን ማደስ አለባቸው። speedup.【scripts/fastpq/wrap_benchmark.py:732】 ለዳሽቦርድ ወይም CI deltas ለብቻው የቆመ JSON ሲፈልጉ `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` ን ያሂዱ; ረዳቱ ሁለቱንም የታሸጉ ቅርሶችን እና ጥሬ የ`fastpq_metal_bench*.json` ቀረጻዎችን ይቀበላል፣ `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` በነባሪ/ሚዛናዊ የጊዜ ሰአቶች፣ ሜታዳታ በማስተካከል እና የተቀዳ ፍጥነት።【scripts/fastpq/export_poseidon_microbench.py:1】
  የ`cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`ን በመተግበር ሩጫውን ያጠናቅቁ ስለዚህ የStage6 የመልቀቅ ማረጋገጫ ዝርዝሩ የ `<1 s` LDE ጣሪያ ያስፈጽማል እና ከተለቀቀው ጋር የሚላክ የተፈረመ አንጸባራቂ/መፍጨት ጥቅል ያወጣል። ቲኬት።【xtask/src/fastpq.rs:1】【አርቲፊክስ/fastpq_benchmarks/README.md:65】
4. ከመልቀቅዎ በፊት ቴሌሜትሪ ያረጋግጡ፡ የPrometheus የመጨረሻ ነጥብ (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) ይንጠፍጡ እና `telemetry::fastpq.execution_mode` ላልተጠበቀ የ`resolved="cpu"` ምዝግቦችን ይፈትሹ። ግቤቶች።【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. የሲፒዩ መመለሻ ዱካውን ሆን ብሎ በማስገደድ (`FASTPQ_GPU=cpu` ወይም `zk.fastpq.execution_mode = "cpu"`) በመመዝገብ የSRE ጫወታ መጽሐፍት ከመወሰኛ ጋር ይጣጣማሉ። behaviour.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. አማራጭ ማስተካከያ፡ በነባሪነት አስተናጋጁ 16 መስመሮችን ለአጭር ዱካዎች፣ 32 ለመካከለኛ እና 64/128 አንድ ጊዜ `log_len ≥ 10/14` ይመርጣል፣ 256 ላይ የሚያርፈው `log_len ≥ 18` ሲሆን አሁን የጋራ የማስታወሻ ንጣፍን በአምስት ደረጃዎች፣0 NI አንድ ጊዜ አራት ትራሴ 8010 12/14/16 ደረጃዎች ለ `log_len ≥ 18/20/22` ወደ ድህረ-ቲሊንግ ከርነል ስራ ከመጀመሩ በፊት. እነዚያን ሂውሪስቲክስ ለመሻር ከላይ ያሉትን ደረጃዎች ከማስኬድዎ በፊት `FASTPQ_METAL_FFT_LANES` (የሁለት ሃይል በ8እና256 መካከል) እና/ወይም `FASTPQ_METAL_FFT_TILE_STAGES` (1-16) ወደ ውጭ ይላኩ። ሁለቱም የ FFT/IFFT እና LDE ዓምድ ባች መጠኖች የተገኘው ከተፈታው የክር ቡድን ስፋት ነው (≈2048 አመክንዮአዊ ክሮች በአንድ መላኪያ፣ በ32 ዓምዶች ተሸፍኗል፣ እና አሁን ጎራው ሲያድግ 32→16→8→4→2→1 እስከ 32 → 16→8→4→2→1 ድረስ እየወረደ ነው) የ LDE ዱካ አሁንም የጎራውን ክዳን ያስፈጽማል። የሚወስነውን የFFT ባች መጠን ለመሰካት `FASTPQ_METAL_FFT_COLUMNS` (1–32) ያቀናብሩ እና `FASTPQ_METAL_LDE_COLUMNS` (1–32) በአስተናጋጆች መካከል የቢት-ለ-ቢት ንፅፅር ሲፈልጉ ተመሳሳይ መሻርን በኤልዲኢ አስተላላፊው ላይ ይተግብሩ። የኤልዲኢ ሰድር ጥልቀት የኤፍኤፍቲ ሂዩሪስቲክስንም ያንፀባርቃል - ከ `log₂ ≥ 18/20/22` ጋር ያሉ ዱካዎች ሰፊውን ቢራቢሮዎች ወደ ድህረ-እርሻ ከመስጠትዎ በፊት 12/10/8 የጋራ ትውስታ ደረጃዎችን ብቻ ያካሂዳሉ - እና ያንን ገደብ በ `FASTPQ_METAL_LDE_TILE_STAGES` (1-32) መሻር ይችላሉ። የሩጫ ጊዜው ሁሉንም እሴቶች በMetal kernel args በኩል ይከርክታል፣ የማይደገፉ መሻሪያዎችን ያቆማል፣ እና የተፈቱትን እሴቶች ይመዘግባል፣ በዚህም ሙከራዎች ሜታሊብ ሳይገነቡ እንደገና ሊባዙ የሚችሉ ናቸው። ቤንችማርክ JSON ሁለቱንም የተፈታውን ማስተካከያ እና የአስተናጋጁ ዜሮ ሙላ በጀት (`zero_fill.{bytes,ms,queue_delta}`) በLDE ስታቲስቲክስ በኩል ተይዟል ስለዚህ ወረፋ ዴልታዎች ከእያንዳንዱ ቀረጻ ጋር በቀጥታ የተሳሰሩ ናቸው፣ እና አሁን የ `column_staging` ብሎክን ይጨምራል (ባችች ጠፍጣፋ፣ ጠፍጣፋ፣ ተጠባቂዎች) አስተናጋጁን አረጋግጠዋል በድርብ የታሸገ የቧንቧ መስመር. ጂፒዩ ዜሮ-ሙላ ቴሌሜትሪ ሪፖርት ለማድረግ ፈቃደኛ ካልሆነ ፣ ማሰሪያው አሁን ከአስተናጋጅ-ጎን ቋት ማጽዳት የሚወስን ጊዜን ያቀናጃል እና ወደ `zero_fill` ብሎክ ውስጥ ያስገባዋል ስለሆነም ማስረጃው ያለ ምንም አይርከብም መስክ።【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. ባለብዙ ወረፋ መላክ በልዩ ማክ ላይ አውቶማቲክ ነው፡- `Device::is_low_power()` የውሸት ሲመለስ ወይም የብረታ ብረት መሳሪያው ማስገቢያ/የውጭ ቦታን ሲዘግብ አስተናጋጁ ሁለት `MTLCommandQueue` ዎችን ያፋጥናል፣የስራው ጫና ≥16 አምዶችን ሲሸከም (በደጋፊዎች የተመጣጠነ በሁለቱም ባንኮቹ ዙሪያ) ሲወጣ ብቻ ነው። የጂፒዩ መስመሮች ቆራጥነትን ሳያበላሹ ስራ በዝተዋል። በ`FASTPQ_METAL_QUEUE_FANOUT` (1-4 ወረፋዎች) እና `FASTPQ_METAL_COLUMN_THRESHOLD` (ከማራገቢያ መውጣት በፊት ቢያንስ ጠቅላላ አምዶች) በማሽኑ ላይ ሊባዙ የሚችሉ ምስሎችን በሚፈልጉበት ጊዜ ፖሊሲውን ይሽሩ። የብዝሃ-ጂፒዩ ማክስ ሽፋን እንዲቆይ እና የተፈታው የደጋፊ መውጫ/ገደብ ከወረፋ-ጥልቀት ቀጥሎ እንዲገባ የነጠላነት ሙከራዎች እነዚያን መሻር ያስገድዳቸዋል። telemetry.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### ማስረጃ ለማህደር
| Artefact | ማንሳት | ማስታወሻ |
|-------|---------|-------|
| `.metallib` ጥቅል | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` እና `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` በመቀጠል `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` እና `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`። | Metal CLI/Toolchain መጫኑን እና ለዚህ ቁርጠኝነት የሚወስን ቤተመጻሕፍት እንዳዘጋጀ ያረጋግጣል።【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| የአካባቢ ቅጽበታዊ ገጽ እይታ | `echo $FASTPQ_METAL_LIB` ከግንባታው በኋላ; በመልቀቂያ ትኬትዎ ትክክለኛውን መንገድ ይያዙ። | ባዶ ውፅዓት ማለት ብረት ተሰናክሏል ማለት ነው። የጂፒዩ መስመሮች በእቃ ማጓጓዣው አርቲፊሻል ላይ የሚገኙትን እሴት ሰነዶች መመዝገብ።【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| የጂፒዩ እኩልነት መዝገብ | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` እና `backend="metal"` ወይም የማውረድ ማስጠንቀቂያ የያዘውን ቅንጣቢ በማህደር ያስቀምጡ። | ግንባታውን ከማስተዋወቅዎ በፊት ከርነሎች እንደሚሮጡ (ወይም በቆራጥነት ወደ ኋላ እንደሚመለሱ) ያሳያል።
| የቤንችማርክ ውጤት | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; በ `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` ተጠቅልለው ይፈርሙ። | የተጠቀለለው JSON መዛግብት `speedup.ratio`፣ `speedup.delta_ms`፣ FFT ማስተካከያ፣ የታሸጉ ረድፎች (32,768)፣ የበለፀጉ `zero_fill`/`kernel_profiles`፣ የተዘረጋው I181NI300000 `metal_dispatch_queue.poseidon`/`poseidon_profiles` ብሎኮች (`--operation poseidon_hash_columns` ጥቅም ላይ ሲውል)፣ እና የመከታተያ ሜታዳታ ስለዚህ ጂፒዩ LDE ማለት ≤950ms ይቆያል እና Poseidon <1s ይቆያል። ዳሽቦርዶች እና ኦዲተሮች ድጋሚ ሳይደረጉ ቅርፁን ማረጋገጥ እንዲችሉ ቅርቅቡን እና የተፈጠረውን `.json.asc` ፊርማ ከመልቀቂያ ትኬት ጋር ያቆዩ። የሥራ ጫና
| የቤንች መግለጫ | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | ሁለቱንም የጂፒዩ ቅርሶች ያረጋግጣል፣ LDE ማለት የ`<1 s` ጣሪያን ከጣሰ፣ BLAKE3/SHA-256 መጭመቂያዎችን መዝግቦ እና የተፈረመ ማኒፌክት ያወጣል ስለዚህ የመልቀቂያ ማረጋገጫ ዝርዝሩ ሊረጋገጥ ካልቻለ አይሳካም። ሜትሪክስ.【xtask/src/fastpq.rs:1】【አርቲፊክስ/fastpq_benchmarks/README.md:65】 |
| CUDA ጥቅል | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`ን በSM80 ላብራቶሪ አስተናጋጅ ላይ ያሂዱ፣ JSON ን ጠቅልለው/ይፈርሙ ወደ `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (`--label device_class=xeon-rtx-sm80` ይጠቀሙ ስለዚህ ዳሽቦርዶች ትክክለኛውን ክፍል እንዲወስዱ)፣ መንገዱን ወደ `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` ይጨምሩ እና I181NIX140007 ን ያቆዩ። አንጸባራቂውን ከማደስዎ በፊት ከብረታ ብረት ጋር ያጣምሩ። ተመዝግቦ የገባው `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` ትክክለኛ የጥቅል ቅርጸት ኦዲተሮች እንደሚጠብቁ ያሳያል።
| ቴሌሜትሪ ማረጋገጫ | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` እና የ `telemetry::fastpq.execution_mode` ምዝግብ ማስታወሻ ሲጀመር። | ትራፊክን ከማስቻሉ በፊት Prometheus/OTEL የሚያጋልጥ `device_class="<matrix>", backend="metal"` (ወይም የወረደ መዝገብ) ያረጋግጣል።| የግዳጅ ሲፒዩ መሰርሰሪያ | አጭር ባች በ `FASTPQ_GPU=cpu` ወይም `zk.fastpq.execution_mode = "cpu"` ያሂዱ እና የማውረድ ምዝግብ ማስታወሻውን ይቅረጹ። | መልሶ መመለሻ በመልቀቅ መሃል ካስፈለገ የSRE runbooksን ከሚወስነው የመመለሻ መንገድ ጋር ያቆያቸዋል።【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| መከታተያ ቀረጻ (አማራጭ) | በ`FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` የተመጣጣኝ ሙከራን ይድገሙት እና የሚወጣውን የመላኪያ ዱካ ያስቀምጡ። | የቆይታ/የክር ቡድን ማስረጃዎችን እንደገና መተኪያ ሳያደርጉ ለበኋላ ለመገለጫ ግምገማዎች ይጠብቃል።【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

የባለብዙ ቋንቋዎች `fastpq_plan.*` ፋይሎች ይህንን የማረጋገጫ ዝርዝር ይጠቅሳሉ ስለዚህ ዝግጅት እና የምርት ኦፕሬተሮች ተመሳሳይ የማስረጃ ዱካ ይከተላሉ።【docs/source/fastpq_plan.md:1】

## ሊባዙ የሚችሉ ግንባታዎች
ሊባዙ የሚችሉ የStage6 ቅርሶችን ለማምረት በተሰካው ኮንቴይነር የስራ ፍሰት ይጠቀሙ፡-

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

የረዳት ስክሪፕት የ `rust:1.88.0-slim-bookworm` የመሳሪያ ሰንሰለት ምስልን (እና `nvidia/cuda:12.2.2-devel-ubuntu22.04` ለጂፒዩ) ግንባታውን በእቃ መያዣው ውስጥ ያካሂዳል እና `manifest.json`፣ `sha256s.txt` እና የተቀናበረውን ሁለትዮሽ ወደ ዒላማው ውጤት ይጽፋል። ማውጫ።【scripts/ fastpq/repro_build.sh:1】【ስክሪፕቶች/ fastpq/run_inside_repro_build.sh:1】【ስክሪፕቶች/ fastpq/docker/Dockerfile.gpu:1】

አካባቢ ይሽራል፡-
- `FASTPQ_RUST_IMAGE`፣ `FASTPQ_RUST_TOOLCHAIN` - ግልጽ የሆነ ዝገት መሰረት/መለያ ይሰኩት።
- `FASTPQ_CUDA_IMAGE` - የጂፒዩ ቅርሶችን በሚያመርቱበት ጊዜ የCUDA መሰረትን ይቀይሩ።
- `FASTPQ_CONTAINER_RUNTIME` - የተወሰነ ጊዜን ማስገደድ; ነባሪ `auto` `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` ይሞክራል።
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` - ለሩጫ ጊዜ ራስ-ማወቂያ (የ `docker,podman,nerdctl` ነባሪዎች) በነጠላ ሰረዝ የተለየ ምርጫ ትእዛዝ።

## የማዋቀር ዝመናዎች
1. በእርስዎ TOML ውስጥ የአሂድ ጊዜ ማስፈጸሚያ ሁነታን ያዘጋጁ፡-
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   እሴቱ በ`FastpqExecutionMode` ተተነተነ እና በሚነሳበት ጊዜ ክሮች ወደ የጀርባው ክፍል ውስጥ ይገባል።【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. ካስፈለገ ይሽሩት፡-
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI የተፈታውን ውቅረት ከመስቀያው ቡት ጫማዎች በፊት ይለውጠዋል።【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. ገንቢዎች ወደ ውጭ በመላክ ውቅሮችን ሳይነኩ ለጊዜው ማወቅን ማስገደድ ይችላሉ።
   ሁለትዮሽውን ከመጀመሩ በፊት `FASTPQ_GPU={auto,cpu,gpu}`; መሻሩ ተዘግቷል እና የቧንቧ መስመር
   አሁንም የተፈታ ሁነታን ያሳያል።【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## የማረጋገጫ ዝርዝር
1. **የጀማሪ ምዝግብ ማስታወሻዎች**
   - ከዒላማ `telemetry::fastpq.execution_mode` ጋር `FASTPQ execution mode resolved` ይጠብቁ
     `requested`፣ `resolved`፣ እና `backend` መለያዎች።【crates/fastpq_prover/src/backend.rs:208】
   - በራስ-ሰር ጂፒዩ ማወቂያ ላይ ከ`fastpq::planner` ሁለተኛ ምዝግብ ማስታወሻ የመጨረሻውን መስመር ሪፖርት ያደርጋል።
   - ሜታልሊብ በተሳካ ሁኔታ ሲጭን `backend="metal"` ላዩን ያስተናግዳል; ማጠናቀር ወይም መጫን ካልተሳካ የግንባታ ስክሪፕቱ ማስጠንቀቂያ ይሰጣል፣ `FASTPQ_METAL_LIB` ያጸዳል እና እቅድ አውጪው ከመቆየቱ በፊት `GPU acceleration unavailable` ይመዘግባል። ሲፒዩ፡- crates/ fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src42. **Prometheus ሜትሪክስ**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   ቆጣሪው በ`record_fastpq_execution_mode` ተጨምሯል (አሁን በ
   `{device_class,chip_family,gpu_kind}`) መስቀለኛ መንገድ አፈፃፀሙን ሲፈታ
   ሁነታ።【crates/iroha_telemetry/src/metrics.rs:8887】
   - ለብረት ሽፋን ማረጋገጫ
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     ከማሰማራት ዳሽቦርዶችዎ ጋር ይጨምራል።【crates/iroha_telemetry/src/metrics.rs:5397】
   - ከ`irohad --features fastpq-gpu` ጋር የተጠናከረ የማክኦኤስ ኖዶች በተጨማሪ ያጋልጣሉ
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     እና
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` ስለዚህ ደረጃ7 ዳሽቦርዶች
     ከቀጥታ Prometheus scraps የግዴታ-ዑደት እና ወረፋ ዋና ክፍል መከታተል ይችላል።

3. **ቴሌሜትሪ ወደ ውጪ መላክ**
   - OTEL ኢሚት `fastpq.execution_mode_resolutions_total` በተመሳሳይ መለያዎች ይገነባል; ያረጋግጡ
     ዳሽቦርዶች ወይም ማንቂያዎች ጂፒዩዎች ንቁ መሆን ሲገባቸው ያልተጠበቀ `resolved="cpu"` ይመለከታሉ።

4. **ንፅህና ማረጋገጫ/አረጋግጥ**
   - ትንሽ ባች በ `iroha_cli` ወይም በውህደት ማሰሪያ በኩል ያሂዱ እና ማረጋገጫዎችን በ
     እኩያ ከተመሳሳዩ መለኪያዎች ጋር የተጠናከረ።

## መላ መፈለግ
- **የተፈታ ሁነታ ሲፒዩ በጂፒዩ አስተናጋጆች ላይ ይቆያል *** - ሁለትዮሽ አብሮ የተሰራ መሆኑን ያረጋግጡ
  `fastpq_prover/fastpq-gpu`፣ CUDA ቤተ-መጻሕፍት በጫኚው መንገድ ላይ ናቸው፣ እና `FASTPQ_GPU` አያስገድድም
  `cpu`.
- ** ብረት በአፕል ሲሊኮን ላይ አይገኝም *** - የ CLI መሳሪያዎች መጫኑን ያረጋግጡ (`xcode-select --install`) ፣ `xcodebuild -downloadComponent MetalToolchain` እንደገና ያሂዱ እና ግንባታው ባዶ ያልሆነ `FASTPQ_METAL_LIB` መንገድ መፈጠሩን ያረጋግጡ ። ባዶ ወይም የጎደለ ዋጋ የጀርባውን ክፍል በንድፍ ያሰናክላል።【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
** `Unknown parameter` ስህተቶች *** - ሁለቱም አረጋጋጭ እና አረጋጋጭ ተመሳሳይ ቀኖናዊ ካታሎግ መጠቀማቸውን ያረጋግጡ
  በ `fastpq_isi` የተለቀቀ; ከ `Error::UnknownParameter` ጋር የማይዛመድ።【crates/fastpq_prover/src/proof.rs:133】
- ** ያልተጠበቀ የሲፒዩ ውድቀት *** - `cargo tree -p fastpq_prover --features` ን ይፈትሹ እና
  አረጋግጥ `fastpq_prover/fastpq-gpu` በጂፒዩ ግንባታዎች ውስጥ አለ; ያረጋግጡ `nvcc`/CUDA ቤተ-ፍርግሞች በፍለጋ መንገዱ ላይ ናቸው።
- **የቴሌሜትሪ ቆጣሪ ጠፍቷል *** - መስቀለኛ መንገድ በ`--features telemetry` (በነባሪ) መጀመሩን ያረጋግጡ።
  እና OTEL ወደ ውጭ መላክ (ከነቃ) የሜትሪክ ቧንቧ መስመርን ያካትታል።【crates/iroha_telemetry/src/metrics.rs:8887】

## የመውደቅ ሂደት
የሚወስነው የቦታ ያዥ ጀርባ ተወግዷል። የድጋፍ መልሶ መመለስ የሚያስፈልገው ከሆነ፣
ከዚህ ቀደም የታወቁትን ጥሩ የተለቀቁ ቅርሶችን እንደገና ማሰማራት እና ደረጃ6 እንደገና ከመውጣቱ በፊት መርምር
ሁለትዮሽ. የለውጡን አስተዳደር ውሳኔ በሰነድ ይመዝግቡ እና የተላለፈው ዝርዝር ከተጠናቀቀ በኋላ ብቻ መጠናቀቁን ያረጋግጡ
መመለሱ ተረድቷል።

3. `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` የሚጠበቀውን እንደሚያንጸባርቅ ለማረጋገጥ ቴሌሜትሪ ይቆጣጠሩ
   ቦታ ያዥ ማስፈጸሚያ.

## የሃርድዌር መነሻ መስመር
| መገለጫ | ሲፒዩ | ጂፒዩ | ማስታወሻ |
| ------- | --- | --- | --- |
| ማጣቀሻ (ደረጃ 6) | AMD EPYC7B12 (32 ኮሮች), ​​256GiB ራም | NVIDIA A10040GB (CUDA12.2) | 20000 ረድፎች ሰራሽ ባች ≤1000ms ማጠናቀቅ አለባቸው።【docs/source/fastpq_plan.md:131】 |
| ሲፒዩ-ብቻ | ≥32 አካላዊ ኮሮች, AVX2 | – | ለ 20000 ረድፎች ~ 0.9-1.2s ይጠብቁ; ለቆራጥነት `execution_mode = "cpu"` ጠብቅ. |## የማገገም ሙከራዎች
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (በጂፒዩ አስተናጋጆች ላይ)
- አማራጭ ወርቃማ ቋሚ ቼክ;
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

ከዚህ የማረጋገጫ ዝርዝር ውስጥ ያሉ ማናቸውንም ልዩነቶችን በኦፕስ runbook ውስጥ ይመዝግቡ እና ከዚያ በኋላ `status.md` ያዘምኑ
የስደት መስኮት ተጠናቀቀ።