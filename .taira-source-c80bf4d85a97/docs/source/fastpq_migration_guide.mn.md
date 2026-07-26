---
lang: mn
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ үйлдвэрлэлийн шилжилт хөдөлгөөний гарын авлага

Энэхүү runbook нь Stage6 үйлдвэрлэлийн FASTPQ проверийг хэрхэн баталгаажуулахыг тайлбарладаг.
Энэ шилжих төлөвлөгөөний нэг хэсэг болгон тодорхойлогч орлуулагчийн арын хэсгийг устгасан.
Энэ нь `docs/source/fastpq_plan.md` дахь үе шаттай төлөвлөгөөг нөхөж, таныг аль хэдийн дагаж мөрддөг гэж үздэг.
`status.md` дахь ажлын талбарын төлөв.

## Үзэгчид ба хамрах хүрээ
- Баталгаажуулагчийн операторууд үйлдвэрлэлийн проверийг үе шат эсвэл үндсэн сүлжээний орчинд нэвтрүүлж байна.
- Үйлдвэрлэлийн арын хэсэгтэй нийлүүлэх хоёртын файл эсвэл контейнер үүсгэгч инженерүүдийг чөлөөл.
- SRE/ажиглалтын багууд телеметрийн шинэ дохиог холбож, сэрэмжлүүлэх.

Хамрах хүрээнээс гадуур: Kotodama гэрээ байгуулах болон IVM ABI өөрчлөлтүүд (`docs/source/nexus.md`-г үзнэ үү.
гүйцэтгэх загвар).

## Онцлог матриц
| Зам | Идэвхжүүлэх ачааны онцлогууд | Үр дүн | Хэзээ хэрэглэх вэ |
| ---- | --------------------------------- | ------ | ----------- |
| Үйлдвэрлэлийн провер (анхдагч) | _байхгүй_ | FFT/LDE төлөвлөгч болон DEEP-FRI дамжуулах хоолой бүхий 6-р шатны FASTPQ backend.【crates/fastpq_prover/src/backend.rs:1144】 | Бүх үйлдвэрлэлийн хоёртын файлуудын өгөгдмөл. |
| Нэмэлт GPU хурдатгал | `fastpq_prover/fastpq-gpu` | CUDA/Metal kernels-ийг CPU-ийн автомат нөөцтэй идэвхжүүлдэг.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Дэмжигдсэн хурдасгууртай хостууд. |

## Барилгын журам
1. **Зөвхөн CPU-д зориулсан бүтээх**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   Үйлдвэрлэлийн арын хэсгийг анхдагчаар хөрвүүлдэг; нэмэлт функц шаардлагагүй.

2. **GPU-г идэвхжүүлсэн бүтээц (заавал биш)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU-г дэмжихийн тулд бүтээх явцад ашиглах боломжтой `nvcc` бүхий SM80+ CUDA хэрэгсэл шаардлагатай.【crates/fastpq_prover/Cargo.toml:11】

3. **Өөрийгөө шалгах**
   ```bash
   cargo test -p fastpq_prover
   ```
   Савлахын өмнө Stage6 замыг баталгаажуулахын тулд үүнийг хувилбар бүрд нэг удаа ажиллуул.

### Металл оосор бэлтгэх (macOS)
1. Барилгын өмнө Метал командын мөрийн хэрэгслүүдийг суулгана уу: `xcode-select --install` (хэрэв CLI хэрэгсэл байхгүй бол) болон `xcodebuild -downloadComponent MetalToolchain` GPU хэрэгслийн хэлхээг татаж авна уу. Бүтээлийн скрипт нь `xcrun metal`/`xcrun metallib`-г шууд дууддаг бөгөөд хэрэв хоёртын файл байхгүй бол хурдан бүтэлгүйтдэг.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. CI-ээс өмнө дамжуулах шугамыг баталгаажуулахын тулд та бүтээх скриптийг дотооддоо толин тусгах боломжтой:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Үүнийг амжилттай хийснээр `FASTPQ_METAL_LIB=<path>` ялгаруулна; ажиллах хугацаа нь металлибыг тодорхой хэмжээгээр ачаалахын тулд энэ утгыг уншдаг.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Металл хэрэгслийн гинжгүйгээр хөндлөн эмхэтгэх үед `FASTPQ_SKIP_GPU_BUILD=1`-г тохируулах; бүтээх нь анхааруулга хэвлэх ба төлөвлөгч нь CPU-ийн зам дээр үлдэнэ.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Метал ажиллахгүй бол зангилаанууд автоматаар CPU-д буцаж унадаг (фрэймворк байхгүй, GPU дэмжигдээгүй эсвэл хоосон `FASTPQ_METAL_LIB`); бүтээх скрипт нь env var-г цэвэрлэж, төлөвлөгч нь бууралтыг бүртгэдэг.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:65】### Хувилбарыг шалгах хуудас (6-р шат)
Доорх зүйл бүрийг бөглөж, хавсаргах хүртэл FASTPQ хувилбарын тасалбарыг блокло.

1. **Дэд секундын баталгаа хэмжигдэхүүн** — Шинээр баригдсан `fastpq_metal_bench_*.json` болон
   `operation = "lde"` (мөн толин тусгалтай) `benchmarks.operations` оруулгыг баталгаажуулна уу.
   `report.operations` дээж) 20000 эгнээний ажлын ачааллын (32768 жийргэвчтэй) `gpu_mean_ms ≤ 950` тайлангууд
   мөр). Таазны гаднах зураг авалтыг шалгах хуудаст гарын үсэг зурахаас өмнө давтан хийх шаардлагатай.
2. **Гарын үсэгтэй манифест** — Гүй
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   Тиймээс суллах тасалбар нь манифест болон түүний салангид гарын үсгийг хоёуланг нь агуулна
   (`artifacts/fastpq_bench_manifest.sig`). Шүүмжлэгч нар өмнө нь тойм/гарын үсэг хосыг баталгаажуулдаг
   хувилбарыг дэмжих.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 Матрицын манифест (барьсан)
   `scripts/fastpq/capture_matrix.sh`-ээр) аль хэдийн 20к эгнээний давхарыг кодлосон ба
   регрессийн дибаг хийх.
3. **Нотлох баримтын хавсралт** — Металл жишиг JSON, stdout бүртгэл (эсвэл Instruments trace)-г байршуулах,
   CUDA/Металл манифестын гаралт, мөн тасалбарын тасалбарын гарын үсэг. Хяналтын хуудасны оруулга
   бүх олдворууд дээр холбогдох аудитын гарын үсэг зурахад ашигладаг нийтийн түлхүүрийн хурууны хээтэй холбох ёстой
   баталгаажуулах алхамыг дахин тоглуулж болно.【artifacts/fastpq_benchmarks/README.md:65】### Металл баталгаажуулалтын ажлын урсгал
1. GPU-г идэвхжүүлсний дараа `FASTPQ_METAL_LIB` цэгүүдийг `.metallib` (`echo $FASTPQ_METAL_LIB`) дээр баталгаажуулснаар ажиллах хугацаа нь тодорхой хэмжээгээр ачаалагдана.【crates/fastpq_prover/build.rs:188】
2. GPU эгнээг албадан ажиллуулж, тэгш байдлын багцыг ажиллуулна уу:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Арын хэсэг нь Металл цөмүүдийг ажиллуулж, илрүүлэлт амжилтгүй болвол тодорхойлогч CPU-ийн нөөцийг бүртгэнэ.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Хяналтын самбарт зориулсан жишиг дээж авах:\
   эмхэтгэсэн металл номын санг олох (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   `FASTPQ_METAL_LIB`-ээр экспортлоод ажиллуулна\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  Каноник `fastpq-lane-balanced` профайл одоо зураг авалт бүрийг 32,768 мөр (2¹⁵) болгон зөөвөрлөж байгаа тул JSON нь `rows` болон `padded_rows`-ийг Металл LDE хоцрогдолтой хамт явуулдаг; `zero_fill` эсвэл дарааллын тохиргоо нь GPU LDE-г AppleM цуврал хостууд дээрх 950 мс (<1 секунд)-ээс хэтрүүлсэн тохиолдолд зураг авалтыг дахин ажиллуулна уу. Гарсан JSON/логийг бусад хувилбарын нотлох баримтын хамт архивлах; шөнийн MacOS-ийн ажлын урсгал нь ижил гүйлтийг хийж, харьцуулалт хийх зорилгоор олдворуудаа байршуулдаг.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Хэрэв танд зөвхөн Poseidon-д зориулсан телеметр хэрэгтэй бол (жишээ нь, Instruments-ийн ул мөрийг бичих) дээрх команд руу `--operation poseidon_hash_columns` нэмнэ үү; вандан сандал нь `FASTPQ_GPU=gpu`-г хүндэтгэсэн хэвээр байх бөгөөд `metal_dispatch_queue.poseidon`-ийг ялгаруулж, шинэ `poseidon_profiles` блокыг багтаасан тул хувилбарын багц нь Poseidon-ийн гацааг тодорхой баримтжуулна.
  Одоо нотлох баримтад `zero_fill.{bytes,ms,queue_delta}` дээр нэмэх нь `kernel_profiles` (цөм тус бүр) багтсан.
  хүн ам, тооцоолсон ГБ/с, үргэлжлэх хугацааны статистик) тул GPU-ийн үр ашгийг ямар ч хамааралгүйгээр графикаар зурах боломжтой.
  түүхий ул мөрийг дахин боловсруулах, мөн `twiddle_cache` блок (онох/алдах + `before_ms`/`after_ms`)
  кэштэй twiddle байршуулалт хүчинтэй байгааг нотолж байна. `--trace-dir` доорх морины оосорыг дахин ажиллуулна
  `xcrun xctrace record` ба
  JSON-ийн хажууд цагийн тэмдэгтэй `.trace` файлыг хадгалдаг; Та захиалгаар өгөх боломжтой хэвээр байна
  `--trace-output` (заавал биш `--trace-template` / `--trace-seconds`-тэй) зураг авах үед
  захиалгат байршил/загвар. JSON нь аудит хийх зорилгоор `metal_trace_{template,seconds,output}` бичлэгийг хийдэг.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Зураг авалт бүрийн дараа `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json`-г ажиллуулсны дараа хэвлэлд Grafana самбар/сэрэмжлүүлэг (`dashboards/grafana/fastpq_acceleration.json`, IVM)-ын хост мета өгөгдлийг (одоо `metadata.metal_trace` орно) авч явдаг. Тайлан нь одоо нэг үйлдэл бүрт `speedup` объектыг (`speedup.ratio`, `speedup.delta_ms`), ороосон өргөгч `zero_fill_hotspots` (байт, хоцролт, үүссэн ГБ/с, Металл дарааллын гурвалжин тоолуурыг I18001X) тэгшлэв. `benchmarks.kernel_summary`, `twiddle_cache` блокийг бүрэн бүтэн байлгаж, шинэ `post_tile_dispatches` блок/хураангуйг хуулж авснаар хянагч олон дамжлагын цөмийг барьж авах явцад ажиллаж байсныг нотлох боломжтой ба одоо Poseidon бичил вандан нотлох баримтыг I10705 самбарт нэгтгэн харуулав. түүхий тайланг дахин задлахгүйгээр скаляр-өгөгдмөл саатал. Манифест хаалга нь ижил блокыг уншиж, үүнийг орхигдуулсан GPU нотлох баримтуудыг үгүйсгэдэг бөгөөд энэ нь хавтангийн дараах замыг алгасах эсвэл бүрмөсөн зураг авалтыг сэргээхэд операторуудыг албаддаг. буруу тохируулсан.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】sk.2st/sc.
  Poseidon2 Металл цөм нь ижил товчлууруудыг хуваалцдаг: `FASTPQ_METAL_POSEIDON_LANES` (32–256, хоёрын хүчин чадал) ба `FASTPQ_METAL_POSEIDON_BATCH` (нэг эгнээ тус бүр 1–32 муж) танд дахин бүтээхгүйгээр хөөргөх өргөн болон эгнээ тус бүрийн ажлыг тогтоох боломжийг олгоно; хост илгээх бүрийн өмнө тэдгээр утгыг `PoseidonArgs`-ээр дамжуулдаг. Анхдагчаар ажиллах хугацаа нь `MTLDevice::{is_low_power,is_headless,location}`-г шалгадаг бөгөөд дискрет GPU-г VRAM шатлалтай хөөргөх рүү чиглүүлдэг (≥48GiB мэдээлсэн үед `256×24`, 32GiB дээр `256×20`, Prometheus бага чадалтай үед бусад тохиолдолд Prometheus) `256×8` (ба түүнээс дээш 128/64 эгнээний хэсгүүд нь нэг эгнээнд 8/6 төлөвт наалддаг) тул ихэнх операторууд хэзээ ч env параметрүүдийг гараар тохируулах шаардлагагүй. `fastpq_metal_bench` нь одоо `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}`-ийн дагуу өөрийгөө дахин ажиллуулж, хөөргөх профайлыг хоёуланг нь бүртгэдэг `poseidon_microbench` блокыг ялгаруулж, скаляр эгнээний хэмжсэн хурдыг бүртгэдэг тул хувилбарын багцууд нь шинэ цөм нь үнэндээ Prometheus-ыг агшааж байгааг батлах боломжтой. `poseidon_pipeline` блок нь 7-р шатын нотолгоо нь хүн амын шинэ түвшний хажуугийн гүн/давхцах товчлууруудыг олж авдаг. Энвийг хэвийн ажиллуулахын тулд тохируулаагүй орхи; бэхэлгээ нь дахин гүйцэтгэлийг автоматаар удирдаж, хүүхдийн зураг авалтыг ажиллуулж чадахгүй бол алдааг бүртгэж, `FASTPQ_GPU=gpu` тохируулагдсан боловч GPU backend байхгүй үед шууд гарна. олдворууд.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】Боодол нь `metal_dispatch_queue.poseidon` дельта, хуваалцсан `column_staging` тоолуур, `poseidon_profiles`/`poseidon_microbench` нотлох блок байхгүй Poseidon-ийн бичлэгийг үгүйсгэдэг тул операторууд бүтэлгүйтсэн, эсвэл алдаатай байгааг нотлохын тулд ямар ч зураг авалтыг сэргээх шаардлагатай. speedup.【scripts/fastpq/wrap_benchmark.py:732】 Хяналтын самбар эсвэл CI гурвалжинд зориулсан бие даасан JSON хэрэгтэй бол `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`-г ажиллуул; Туслагч нь ороосон олдворууд болон түүхий `fastpq_metal_bench*.json` зургийн аль алиныг нь хүлээн авч, `benchmarks/poseidon/poseidon_microbench_<timestamp>.json`-г анхдагч/скаляр цаг хугацаа, тааруулах мета өгөгдөл, бүртгэгдсэн хурдыг ялгаруулдаг.【scripts/fastpq/export_poseidon_py:1crobench.
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`-г ажиллуулснаар гүйлтийг дуусгаснаар 6-р шатны хувилбарын хяналтын хуудас нь `<1 s` LDE дээд хязгаарыг хэрэгжүүлж, хувилбартай хамт ирдэг гарын үсэгтэй манифест/дижест багцыг ялгаруулна. тасалбар.【xtask/src/fastpq.rs:1】【олдвор/fastpq_benchmarks/README.md:65】
4. Дамжуулахын өмнө телеметрийг шалгана уу: Prometheus төгсгөлийн цэгийг (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) нугалж, `telemetry::fastpq.execution_mode` бүртгэлд гэнэтийн `resolved="cpu"` байгаа эсэхийг шалгана уу. оруулгууд.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. CPU-ийн нөөцийн замыг зориудаар хүчээр (`FASTPQ_GPU=cpu` эсвэл `zk.fastpq.execution_mode = "cpu"`) баримтжуулж, SRE тоглоомын номууд нь детерминистиктай нийцэж байх болно. зан байдал.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. Нэмэлт тааруулах: өгөгдмөл байдлаар хост богино мөрийн хувьд 16 эгнээ, дундын хувьд 32, `log_len ≥ 10/14` нэг удаа 64/128, `log_len ≥ 18` үед 256-д буух ба одоо хуваалцсан санах ойн хавтанг таван үе шаттайгаар, дөрвөн удаа I100100000000000 жижиг мөрүүдийг сонгож байна. ба 12/14/16 үе шатууд `log_len ≥ 18/20/22`-ийн ажилыг хавтангийн дараах цөм рүү өшиглөхөөс өмнө. Эдгээр эвристикийг хүчингүй болгохын тулд дээрх алхмуудыг гүйцэтгэхээс өмнө `FASTPQ_METAL_FFT_LANES` (8 ба 256-ийн хооронд хоёрын хүч) ба/эсвэл `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) экспортло. FFT/IFFT болон LDE баганын багцын хэмжээ хоёулаа шийдэгдсэн урсгалын бүлгийн өргөнөөс (диспетчерт ≈2048 логик хэлхээ, 32 баганаар хязгаарлагдсан, одоо домэйн өсөхийн хэрээр 32→16→8→4→2→1-ээр дамждаг) LDE зам нь өөрийн домайн хязгаарыг хэрэгжүүлсээр байгаа; Тодорхойлолттой FFT багцын хэмжээг тогтоохын тулд `FASTPQ_METAL_FFT_COLUMNS` (1–32) тохируулж, хостуудын хооронд битийн харьцуулалт хийх шаардлагатай үед LDE диспетчерт ижил хүчингүй болгохыг ашиглахын тулд `FASTPQ_METAL_LDE_COLUMNS` (1–32) тохируулна. LDE хавтангийн гүн нь FFT эвристикийг тусгадаг—`log₂ ≥ 18/20/22`-тай мөрүүд нь өргөн эрвээхэйг хавтангийн дараах цөмд шилжүүлэхээс өмнө зөвхөн 12/10/8 хуваалцсан санах ойн үе шатуудыг ажиллуулдаг бөгөөд та `FASTPQ_METAL_LDE_TILE_STAGES` (1–32)-ээр дамжуулан энэ хязгаарыг хүчингүй болгож болно. Ажиллах цаг нь бүх утгыг Металл цөмийн аргуудыг дамжуулж, дэмжигдээгүй даралтыг хавчуулж, шийдвэрлэсэн утгуудыг бүртгэдэг тул металибыг дахин бүтээхгүйгээр туршилтууд дахин давтагдах боломжтой хэвээр байх болно; жишиг JSON нь LDE статистикаар авсан тохируулсан тохируулга болон хостын тэг дүүргэлтийн төсвийг (`zero_fill.{bytes,ms,queue_delta}`) хоёуланг нь харуулдаг тул дарааллын дельта нь зураг авалт бүрт шууд холбогддог ба одоо `column_staging` блок нэмсэн (багцуудыг хавтгайруулсан, хавтгайруулсан, хостууд шалгах, хүлээх боломжтой) давхар буферт дамжуулах хоолойгоор нэвтрүүлсэн. GPU нь тэг дүүргэлтийн телеметрийг мэдээлэхээс татгалзах үед утас нь одоо хост талын буферийн цэвэрлэгээнээс тодорхойлогч хугацааг нэгтгэж, `zero_fill` блок руу оруулдаг тул нотлох баримтыг хэзээ ч илгээхгүй талбар.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. Олон дарааллын илгээлт нь салангид Mac дээр автоматаар хийгддэг: `Device::is_low_power()` худал буцах эсвэл Метал төхөөрөмж нь Slot/Гадаад байршлыг мэдээлэх үед хост нь хоёр `MTLCommandQueue`-г үүсгэнэ, ажлын ачаалал ≥16 багана (хэмжээгээр томруулсан) ба фенүүдээр гадагшлуулна. урт ул мөр нь детерминизмыг алдагдуулахгүйгээр GPU хоёр эгнээг завгүй байлгадаг. Машин дээр давтагдах боломжтой зураг авах шаардлагатай үед `FASTPQ_METAL_QUEUE_FANOUT` (1–4 дараалал) болон `FASTPQ_METAL_COLUMN_THRESHOLD` (сэтгэл гаргахаас өмнөх хамгийн бага нийт багана) ашиглан бодлогыг хүчингүй болгох; Паритет тестүүд нь эдгээр хүчингүй болгохыг албаддаг тул олон GPU-тэй Mac-ууд хамгаалагдсан хэвээр байх ба шийдэгдсэн сэнс гарах/босгыг дарааллын гүнийн хажууд бүртгэдэг. телеметр.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### Архивлах нотлох баримт
| Олдвор | Зураг авах | Тэмдэглэл |
|----------|---------|-------|
| `.metallib` багц | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`, `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, дараа нь `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`, `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Металл CLI/toolchain суулгаж, энэ үйлдлийг тодорхойлоход зориулагдсан номын сан үүсгэсэн болохыг баталж байна.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Байгаль орчны агшин зураг | Барилгын дараа `echo $FASTPQ_METAL_LIB`; суллах тасалбараараа үнэмлэхүй замыг хадгал. | Хоосон гаралт нь Метал идэвхгүй болсон гэсэн үг; GPU зурвасууд тээвэрлэлтийн олдвор дээр байгаа хэвээр байгаа үнэ цэнийн баримтуудыг бүртгэж байна.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU паритын бүртгэл | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` ба `backend="metal"` буюу зэрэглэл буурах анхааруулгыг агуулсан хэсгийг архивлана уу. | Бүтээлийг сурталчлахаас өмнө цөмүүд ажилладаг (эсвэл тодорхой хэмжээгээр унадаг) гэдгийг харуулж байна.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Жишиг гаралт | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; боож, `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`-ээр гарын үсэг зурна уу. | Боодолтой JSON бичлэг нь `speedup.ratio`, `speedup.delta_ms`, FFT тааруулах, дэвсгэртэй мөрүүд (32,768), баяжуулсан `zero_fill`/`kernel_profiles`, хавтгайруулсан Prometheus, `metal_dispatch_queue.poseidon`/`poseidon_profiles` блокууд (`--operation poseidon_hash_columns`-г ашиглах үед) ба мета өгөгдлийг хянах нь GPU LDE дундаж нь ≤950ms, Poseidon нь <1s хэвээр байна; Багц болон үүсгэсэн `.json.asc` гарын үсгийг хувилбарын тасалбартай хамт үлдээгээрэй, ингэснээр хяналтын самбар болон аудиторууд дахин ажиллуулахгүйгээр олдворыг шалгах боломжтой болно. ажлын ачаалал.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Вандан манифест | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | GPU олдворуудыг хоёуланг нь баталгаажуулж, LDE дундаж нь `<1 s` дээд хязгаарыг эвдэж, BLAKE3/SHA-256 дижестийг бүртгэж, гарын үсэг зурсан манифест гаргадаг тул баталгаажуулахгүйгээр хувилбарын хяналтын хуудас урагшлахгүй бол амжилтгүй болно. хэмжүүр.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA багц | SM80 лабораторийн хост дээр `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`-г ажиллуулж, JSON-г `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`-д боож/тэмдэглээрэй (хяналтын самбар зөв анги сонгохын тулд `--label device_class=xeon-rtx-sm80`-г ашиглана уу), `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` руу замыг нэмж, `.json`/`.asc` манифестийг сэргээхийн өмнө Металл олдвортой хослуул. Бүртгэгдсэн `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` нь аудиторуудын хүлээж буй багцын форматыг харуулж байна.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80】t |
| Телеметрийн баталгаа | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` дээр нэмэх нь эхлүүлэх үед ялгарсан `telemetry::fastpq.execution_mode` бүртгэл. | Хөдөлгөөнийг идэвхжүүлэхийн өмнө Prometheus/OTEL `device_class="<matrix>", backend="metal"` (эсвэл доошилсон лог)-ыг илрүүлж байгааг баталгаажуулдаг.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/74rs: |1.| CPU-ийн албадан өрөм | `FASTPQ_GPU=cpu` эсвэл `zk.fastpq.execution_mode = "cpu"`-ээр богино багц ажиллуулж, бууралтын бүртгэлийг аваарай. | Хувилбарын дундуур буцаах шаардлагатай тохиолдолд SRE runbook-г тодорхойлогч буцах замтай нийцүүлэн байлгадаг.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Trace capture (заавал биш) | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …`-ээр паритет тестийг давтаж, ялгарсан илгээлтийн ул мөрийг хадгална уу. | Шалгалтуудыг дахин хийхгүйгээр дараа нь профайл гаргахад зориулсан суудлын/threadgroup нотолгоог хадгалдаг.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Олон хэл дээрх `fastpq_plan.*` файлууд нь энэхүү хяналтын хуудсыг иш татдаг тул үе шат болон үйлдвэрлэлийн операторууд ижил нотлох баримтын мөрийг дагаж мөрддөг.【docs/source/fastpq_plan.md:1】

## Хуулбарлах боломжтой бүтээц
Дахин давтагдах боломжтой 6-р шатны олдворуудыг хийхийн тулд зүүсэн савны ажлын урсгалыг ашиглана уу:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Туслах скрипт нь `rust:1.88.0-slim-bookworm` хэрэгслийн гинжний дүрсийг (мөн GPU-д зориулсан `nvidia/cuda:12.2.2-devel-ubuntu22.04`) бүтээж, угсралтыг контейнер дотор ажиллуулж, `manifest.json`, `sha256s.txt`, хөрвүүлсэн хоёртын файлуудыг зорилтот гаралт руу бичдэг. лавлах.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Байгаль орчны хүчин зүйлс:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – тодорхой Rust суурь/шошгийг зүү.
- `FASTPQ_CUDA_IMAGE` – GPU олдворуудыг үйлдвэрлэхдээ CUDA суурийг солих.
- `FASTPQ_CONTAINER_RUNTIME` – тодорхой ажиллах хугацааг албадах; анхдагч `auto` `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` оролддог.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – ажлын цагийг автоматаар илрүүлэх таслалаар тусгаарлагдсан давуу дараалал (өгөгдмөл нь `docker,podman,nerdctl`).

## Тохиргооны шинэчлэлтүүд
1. TOML дээрээ ажиллах цагийн горимыг тохируулна уу:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   Утгыг `FastpqExecutionMode`-ээр задлан шинжилж, эхлүүлэх үед арын хэсэгт оруулна.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Шаардлагатай бол эхлүүлэх үед дарж бичнэ үү:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI нь зангилаа ачаалахаас өмнө шийдвэрлэсэн тохиргоог мутаци хийдэг.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Хөгжүүлэгчид экспортлох замаар тохиргоонд хүрэхгүйгээр түр зуур илрүүлэхийг албадах боломжтой
   Хоёртын файлыг эхлүүлэхийн өмнө `FASTPQ_GPU={auto,cpu,gpu}`; override бүртгэл болон дамжуулах хоолой
   Шийдвэрлэсэн горимыг харуулсан хэвээр байна.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Баталгаажуулах шалгах хуудас
1. **Эхлүүлэх бүртгэл**
   - `telemetry::fastpq.execution_mode` зорилтоос `FASTPQ execution mode resolved`-г хүлээгээрэй.
     `requested`, `resolved`, болон `backend` шошго.【crates/fastpq_prover/src/backend.rs:208】
   - Автомат GPU илрүүлэх үед `fastpq::planner`-ийн хоёрдогч бүртгэл нь эцсийн эгнээний талаар мэдээлдэг.
   - Металлибыг амжилттай ачаалах үед `backend="metal"` гадаргуутай металл хост; Хэрэв эмхэтгэх эсвэл ачаалах үед бүтэлгүйтсэн бол бүтээх скрипт нь анхааруулга өгч, `FASTPQ_METAL_LIB`-г устгаж, төлөвлөгч нь ажиллахаасаа өмнө `GPU acceleration unavailable`-г бүртгэдэг. CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover】43】2. **Prometheus хэмжүүр**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Тоолуурыг `record_fastpq_execution_mode` (одоо
   `{device_class,chip_family,gpu_kind}`) зангилаа гүйцэтгэлээ шийдэх бүрт
   горим.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Металлын хамрах хүрээг баталгаажуулна уу
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     Таны байршуулалтын хяналтын самбарын хажуугаар нэмэгдэнэ.【crates/iroha_telemetry/src/metrics.rs:5397】
   - `irohad --features fastpq-gpu`-ээр эмхэтгэсэн macOS зангилаанууд нэмэлтээр илэрдэг
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     болон
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` тиймээс Stage7 хяналтын самбарууд
     Prometheus шууд зурааснаас ажлын мөчлөг болон дарааллын өндөр зайг хянах боломжтой.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Телеметрийн экспорт**
   - OTEL-ийн хийцүүд ижил шошготой `fastpq.execution_mode_resolutions_total` ялгаруулдаг; таны
     хяналтын самбар эсвэл анхааруулга нь GPU идэвхтэй байх үед гэнэтийн `resolved="cpu"`-ийг хардаг.

4. **Эрүүл мэндийн байдлыг батлах/баталгаажуулах**
   - `iroha_cli` эсвэл интеграцийн оосороор дамжуулан жижиг багцыг ажиллуулж, нотлох баримтуудыг баталгаажуулна уу.
     ижил параметрүүдээр эмхэтгэсэн peer.

## Алдааг олж засварлах
- **Шийдвэрлэсэн горим нь GPU хостууд дээр CPU хэвээр үлдэнэ** — хоёртын файлыг суулгасан эсэхийг шалгана уу
  `fastpq_prover/fastpq-gpu`, CUDA номын сангууд ачаалагчийн зам дээр байгаа бөгөөд `FASTPQ_GPU` албадаагүй байна
  `cpu`.
- **Apple Silicon дээр металл байхгүй** — CLI хэрэгслүүдийг суулгасан эсэхийг шалгах (`xcode-select --install`), `xcodebuild -downloadComponent MetalToolchain` дахин ажиллуулж, уг бүтээц нь хоосон биш `FASTPQ_METAL_LIB` замыг үүсгэсэн эсэхийг баталгаажуулах; хоосон эсвэл дутуу утга нь дизайнаар арын хэсгийг идэвхгүй болгодог.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` алдаа** — нотлогч болон баталгаажуулагч хоёулаа ижил каноник каталог ашигладаг эсэхийг шалгаарай
  `fastpq_isi` ялгаруулсан; гадаргуу нь `Error::UnknownParameter`-тай таарахгүй байна.【crates/fastpq_prover/src/proof.rs:133】
- **Гэнэтийн CPU-ийн уналт** — `cargo tree -p fastpq_prover --features` болон шалгах
  GPU бүтээцэд `fastpq_prover/fastpq-gpu` байгааг баталгаажуулах; `nvcc`/CUDA сангууд хайлтын зам дээр байгаа эсэхийг шалгана уу.
- **Телеметрийн тоолуур дутуу** — зангилаа `--features telemetry` (өгөгдмөл)-ээр эхлүүлсэн эсэхийг шалгах
  мөн OTEL экспорт (хэрэв идэвхжүүлсэн бол) хэмжигдэхүүн дамжуулах шугамыг агуулна.【crates/iroha_telemetry/src/metrics.rs:8887】

## Буцах журам
Тодорхойлогч орлуулагчийн арын хэсгийг устгасан. Хэрэв регресс буцах шаардлагатай бол,
6-р шатыг дахин гаргахаас өмнө өмнө нь мэдэгдэж байсан сайн хувилбаруудыг дахин байршуулж, судлах
хоёртын файлууд. Өөрчлөлтийн менежментийн шийдвэрийг баримтжуулж, зөвхөн дараа нь шилжүүлж дуусгахыг баталгаажуулна уу
регресс гэж ойлгогддог.

3. `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` нь хүлээгдэж буйг тусгахын тулд телеметрийг хяна
   орлуулагчийн гүйцэтгэл.

## Техник хангамжийн суурь үзүүлэлт
| Профайл | CPU | GPU | Тэмдэглэл |
| ------- | --- | --- | ----- |
| Лавлагаа (6-р шат) | AMD EPYC7B12 (32 цөм), 256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000 эгнээний синтетик багц нь ≤1000ms дуусгах ёстой.【docs/source/fastpq_plan.md:131】 |
| Зөвхөн CPU-д зориулагдсан | ≥32 физик цөм, AVX2 | – | 20000 мөрийн хувьд ~0.9–1.2 секунд байх ёстой; Детерминизмын хувьд `execution_mode = "cpu"`-г хадгал. |## Регрессийн тест
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU хостууд дээр)
- Нэмэлт алтан бэхэлгээ шалгах:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Энэ хяналтын жагсаалтаас хазайлтыг өөрийн үйлдлийн дэвтэртээ баримтжуулж, дараа нь `status.md`-г шинэчилнэ үү.
шилжих цонх дуусна.