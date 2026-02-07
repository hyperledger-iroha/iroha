---
lang: uz
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ ishlab chiqarish migratsiyasi bo'yicha qo'llanma

Ushbu runbook Stage6 ishlab chiqarish FASTPQ proverini qanday tekshirishni tasvirlaydi.
Deterministik to'ldiruvchining orqa qismi ushbu ko'chirish rejasining bir qismi sifatida olib tashlandi.
Bu `docs/source/fastpq_plan.md`-dagi bosqichli rejani to'ldiradi va siz allaqachon kuzatib borasiz deb taxmin qiladi.
`status.md` da ish maydoni holati.

## Tomoshabinlar va qamrov
- Ishlab chiqarish proverini staging yoki mainnet muhitida tarqatuvchi tasdiqlovchi operatorlar.
- Ikkilik yoki konteynerlarni yaratuvchi muhandislarni ishlab chiqarish backend bilan birga yuboring.
- SRE/kuzatish guruhlari yangi telemetriya signallarini ulash va ogohlantirish.

Qo'llash doirasi tashqarida: Kotodama shartnoma muallifi va IVM ABI o'zgarishlari (qarang: `docs/source/nexus.md`).
ijro modeli).

## Xususiyatlar matritsasi
| Yo'l | Yoqish uchun yuk xususiyatlari | Natija | Qachon foydalanish kerak |
| ---- | --------------------------------- | ------ | ----------- |
| Ishlab chiqarish proveri (standart) | _yo'q_ | FFT/LDE rejalashtiruvchisi va DEEP-FRI quvur liniyasi bilan Stage6 FASTPQ backend.【crates/fastpq_prover/src/backend.rs:1144】 | Barcha ishlab chiqarish ikkiliklari uchun standart. |
| Ixtiyoriy GPU tezlashtirish | `fastpq_prover/fastpq-gpu` | CUDA/Metal yadrolari protsessorning avtomatik qayta tiklanishiga ega.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Qo'llab-quvvatlanadigan tezlatgichlarga ega xostlar. |

## Qurilish tartibi
1. **Faqat CPU qurish**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   Ishlab chiqarish backend sukut bo'yicha kompilyatsiya qilinadi; qo'shimcha funktsiyalar talab qilinmaydi.

2. **GPU yoqilgan qurilish (ixtiyoriy)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU qo‘llab-quvvatlashi uchun SM80+ CUDA asboblar to‘plami qurilish vaqtida mavjud bo‘lgan `nvcc` kerak.【crates/fastpq_prover/Cargo.toml:11】

3. **O'z-o'zini sinab ko'rish**
   ```bash
   cargo test -p fastpq_prover
   ```
   Qadoqlashdan oldin Stage6 yo'lini tasdiqlash uchun har bir versiyada bir marta ishga tushiring.

### Metall asboblar zanjirini tayyorlash (macOS)
1. Qurilishdan oldin Metall buyruq qatori vositalarini o'rnating: GPU asboblar zanjirini olish uchun `xcode-select --install` (agar CLI asboblari yo'q bo'lsa) va `xcodebuild -downloadComponent MetalToolchain`. Qurilish skripti to'g'ridan-to'g'ri `xcrun metal`/`xcrun metallib` ni chaqiradi va ikkilik fayllar bo'lmasa, tezda ishlamay qoladi.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. CI dan oldingi quvur liniyasini tasdiqlash uchun siz qurish skriptini mahalliy sifatida aks ettirishingiz mumkin:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Bu muvaffaqiyatli bo'lganda, qurish `FASTPQ_METAL_LIB=<path>` chiqaradi; ish vaqti metallibni deterministik tarzda yuklash uchun ushbu qiymatni o'qiydi.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Metall asboblar zanjirisiz o'zaro kompilyatsiya qilishda `FASTPQ_SKIP_GPU_BUILD=1` ni o'rnating; qurish ogohlantirishni chop etadi va rejalashtiruvchi protsessor yo'lida qoladi.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Metall mavjud bo'lmasa (ramka etishmayotgan, qo'llab-quvvatlanmaydigan GPU yoki bo'sh `FASTPQ_METAL_LIB`) tugunlar avtomatik ravishda protsessorga qaytadi; qurish skripti env var faylini tozalaydi va rejalashtiruvchi pasaytirishni qayd qiladi.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:60】### Nashr ro'yxati (6-bosqich)
Quyidagi har bir element toʻliq va biriktirilgunga qadar FASTPQ reliz chiptasini bloklangan holda saqlang.

1. **Sub-ikkinchi isbot ko'rsatkichlari** — Yangi olingan `fastpq_metal_bench_*.json` va
   `benchmarks.operations` yozuvini tasdiqlang, bu erda `operation = "lde"` (va aks ettirilgan)
   `report.operations` namunasi) 20000 qatorli ish yuki (32768 toʻldirilgan) uchun `gpu_mean_ms ≤ 950` hisoboti
   qatorlar). Shiftdan tashqarida suratga olishlar nazorat ro'yxati imzolanishidan oldin takrorlashni talab qiladi.
2. **Imzolangan manifest** — Yugurish
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   shuning uchun chiqish chiptasi manifestni ham, uning alohida imzosini ham o'z ichiga oladi
   (`artifacts/fastpq_bench_manifest.sig`). Sharhlovchilar oldin dayjest/imzo juftligini tekshiradilar
   chiqarishni targ'ib qilish.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 Matritsa manifesti (qurilgan)
   `scripts/fastpq/capture_matrix.sh` orqali) allaqachon 20k qatorli qavatni kodlaydi va
   regressiyani tuzatish.
3. **Dalil qo‘shimchalari** — Metall benchmark JSON, stdout log (yoki Instruments trace) ni yuklang.
   CUDA/Metal manifest chiqishlari va chiqish chiptasining ajratilgan imzosi. Tekshirish ro'yxatiga kirish
   barcha artefaktlarga hamda quyi oqimdagi auditlarni imzolash uchun foydalaniladigan ochiq kalit barmoq iziga bog‘lanishi kerak
   tekshirish bosqichini takrorlashi mumkin.【artifacts/fastpq_benchmarks/README.md:65】### Metallni tekshirish ish jarayoni
1. GPU yoqilgan qurilishdan so'ng, `FASTPQ_METAL_LIB` nuqtalarini `.metallib` (`echo $FASTPQ_METAL_LIB`) da tasdiqlang, shunda ish vaqti uni aniq yuklashi mumkin.【crates/fastpq_prover/build.rs:188】
2. Paritet to‘plamini GPU qatorlari majburiy yoqilgan holda ishga tushiring:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Backend metall yadrolarini ishlatadi va agar aniqlash muvaffaqiyatsiz bo'lsa, deterministik protsessorni qayta tiklashni qayd qiladi.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Boshqaruv panellari uchun benchmark namunasini oling:\
   tuzilgan metall kutubxonani toping (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   uni `FASTPQ_METAL_LIB` orqali eksport qiling va ishga tushiring
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  Kanonik `fastpq-lane-balanced` profili endi har bir suratga olishni 32 768 qatorga (2¹⁵) yopadi, shuning uchun JSON ikkala `rows` va `padded_rows` va Metall LDE kechikishini tashiydi; Agar `zero_fill` yoki navbat sozlamalari GPU LDE-ni AppleM-seriyali xostlarda 950ms (<1s) nishonidan oshib ketsa, suratga olishni qayta ishga tushiring. Olingan JSON/jurnalni boshqa nashr dalillari bilan birga arxivlang; tungi macOS ish jarayoni bir xil ishga tushirishni amalga oshiradi va taqqoslash uchun artefaktlarini yuklaydi.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Sizga faqat Poseidon telemetriyasi kerak bo'lganda (masalan, Instruments izini yozish uchun), yuqoridagi buyruqqa `--operation poseidon_hash_columns` ni qo'shing; dastgoh hali ham `FASTPQ_GPU=gpu` ni hurmat qiladi, `metal_dispatch_queue.poseidon` chiqaradi va yangi `poseidon_profiles` blokini o'z ichiga oladi, shuning uchun relizlar to'plami Poseidon bo'g'inini aniq hujjatlashtiradi.
  Dalillar endi `zero_fill.{bytes,ms,queue_delta}` va `kernel_profiles` (har bir yadro) o'z ichiga oladi
  bandlik, taxminiy GB/s va davomiylik statistikasi), shuning uchun GPU samaradorligini grafiksiz ko'rsatish mumkin.
  xom izlarni qayta ishlash va `twiddle_cache` bloki (hits/o'tkazib yuborish + `before_ms`/`after_ms`)
  keshlangan twiddle yuklamalar amalda ekanligini isbotlaydi. `--trace-dir` ostidagi jabduqni qayta ishga tushiradi
  `xcrun xctrace record` va
  JSON bilan birga vaqt tamg'asi bo'lgan `.trace` faylini saqlaydi; siz hali ham buyurtma berishingiz mumkin
  `--trace-output` (ixtiyoriy `--trace-template` / `--trace-seconds` bilan) suratga olishda
  moslashtirilgan joy/shablon. JSON audit uchun `metal_trace_{template,seconds,output}` yozuvlarini oladi.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Har bir suratga olishdan so‘ng `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` ishga tushiriladi, shuning uchun nashr Grafana doskasi/ogohlantirish to‘plami (`dashboards/grafana/fastpq_acceleration.json`, IVM) uchun xost metama’lumotlarini (hozir `metadata.metal_trace` bilan birga) olib yuradi. Hisobotda endi har bir operatsiya uchun `speedup` ob'ekti (`speedup.ratio`, `speedup.delta_ms`), o'rash ko'targichlari `zero_fill_hotspots` (baytlar, kechikish, olingan GB/s va Metall navbat delta hisoblagichlariga I18001), tekislanadi. `benchmarks.kernel_summary`, `twiddle_cache` blokini buzilmagan holda saqlaydi, yangi `post_tile_dispatches` blokini/xulosasini nusxa ko'chiradi, shunda ko'rib chiquvchilar ko'p o'tishli yadroni qo'lga olish paytida ishlaganligini isbotlashlari mumkin va endi Poseidon mikrobench dalillarini I10NI005 da so'zlab beradi. xom hisobotni qayta ishlamasdan, skalyar va standart kechikish. Manifest eshigi bir xil blokni o'qiydi va uni o'tkazib yuboradigan GPU dalillar to'plamlarini rad etadi, bu esa operatorlarni har safar plitka qo'yish yo'li o'tkazib yuborilganda yoki yangilanishni yangilashga majbur qiladi. noto'g'ri sozlangan.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】sk.2x8】rc.
  Poseidon2 Metal yadrosi bir xil tugmachalarga ega: `FASTPQ_METAL_POSEIDON_LANES` (32–256, ikkita kuch) va `FASTPQ_METAL_POSEIDON_BATCH` (har bir chiziqda 1–32 ta holat) ishga tushirish kengligi va har bir chiziqli ishini qayta qurmasdan belgilash imkonini beradi; xost har bir jo'natishdan oldin ushbu qiymatlarni `PoseidonArgs` orqali uzatadi. Sukut bo'yicha ish vaqti `MTLDevice::{is_low_power,is_headless,location}` diskret GPU-larni VRAM-darajali ishga tushirishga yo'naltirish uchun tekshiradi (≥48GiB xabar berilganda `256×24`, 32GiBda `256×20`, Prometheus esa past quvvatda qoladi) `256×8` (va eski 128/64 chiziqli qismlar har bir qatorda 8/6 holatga yopishadi), shuning uchun ko'pchilik operatorlar hech qachon env parametrlarini qo'lda o'rnatishlari shart emas.【crates/fastpq_prover/src/metal_config.rs:78】【crates/78/sp:78】【crates/fastp.1sp. `fastpq_metal_bench` endi o'zini `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ostida qayta ishga tushiradi va ikkala ishga tushirish profilini va skaler chiziqqa nisbatan o'lchangan tezlikni qayd qiluvchi `poseidon_microbench` blokini chiqaradi, shuning uchun bo'shatish to'plamlari yangi yadro haqiqatda qisqarishini isbotlashi mumkin Prometheus va uni o'z ichiga oladi. `poseidon_pipeline` bloki, shuning uchun Stage7 dalili yangi bandlik darajalari bilan birga bo'lak chuqurligi/bir-biriga yopishish tugmalarini ushlaydi. Oddiy ishlash uchun env ni o'rnatilmagan holda qoldiring; jabduqlar avtomatik ravishda qayta bajarishni boshqaradi, agar bolani suratga olish ishlamasa, xatoliklarni qayd qiladi va `FASTPQ_GPU=gpu` o'rnatilganda darhol chiqadi, lekin GPU backend mavjud emas, shuning uchun jim protsessor zaxiralari hech qachon perfga tushmaydi. artefaktlar.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】Oʻram `metal_dispatch_queue.poseidon` deltasi, umumiy `column_staging` hisoblagichlari yoki `poseidon_profiles`/`poseidon_microbench` dalillar bloklari yetishmayotgan Poseidon suratlarini rad etadi, shuning uchun operatorlar muvaffaqiyatsizliklarni isbotlash uchun har qanday suratga olishni yangilashi kerak. speedup.【scripts/fastpq/wrap_benchmark.py:732】 Agar asboblar paneli yoki CI deltalari uchun mustaqil JSON kerak bo'lsa, `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` ni ishga tushiring; yordamchi oʻralgan artefaktlarni ham, xom `fastpq_metal_bench*.json` tasvirlarini ham qabul qiladi, `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` ni standart/skalar vaqtlar, sozlash metamaʼlumotlari va qayd etilgan tezlik bilan chiqaradi.【scripts/fastpq/export_poseidon_microbench:1.
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` ni bajarish orqali ishga tushirishni yakunlang, shunda Stage6 nashri nazorat roʻyxati `<1 s` LDE shiftini taʼminlaydi va nashr bilan birga kelgan imzolangan manifest/dijest toʻplamini chiqaradi. chipta.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. Chiqarishdan oldin telemetriyani tekshiring: Prometheus oxirgi nuqtasini (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) burang va `telemetry::fastpq.execution_mode` jurnallarida kutilmagan `resolved="cpu"` borligini tekshiring yozuvlar.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. SRE o'yin kitoblari deterministik bilan bir xil bo'lib qolishi uchun uni ataylab majburlash (`FASTPQ_GPU=cpu` yoki `zk.fastpq.execution_mode = "cpu"`) orqali protsessorni qayta tiklash yo'lini hujjatlashtiring. xatti.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. Ixtiyoriy sozlash: sukut boʻyicha xost qisqa izlar uchun 16, oʻrta uchun 32 va bir marta `log_len ≥ 10/14` uchun 64/128 yoʻlni tanlaydi, `log_len ≥ 18` boʻlganda 256-ga tushadi va endi umumiy xotira plitasini besh bosqichda, bir marta toʻrt bosqichda Prometheus olduğumuzimiz uchun tanlaydi. va `log_len ≥ 18/20/22` uchun 12/14/16 bosqichlari plitkadan keyingi yadroga ishni boshlashdan oldin. Ushbu evristik ma'lumotlarni bekor qilish uchun yuqoridagi amallarni bajarishdan oldin `FASTPQ_METAL_FFT_LANES` (ikkining kuchi 8 va 256 orasida) va/yoki `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) eksport qiling. FFT/IFFT va LDE ustunlarining partiyasi o‘lchamlari hal qilingan tarmoq guruhi kengligidan kelib chiqadi (har bir jo‘natma uchun ≈2048 mantiqiy oqim, 32 ta ustun bilan chegaralangan va endi domen o‘sishi bilan 32→16→8→4→2→1 gacha pasayadi), LDE yo‘li esa hali ham o‘z domen cheklarini qo‘llamoqda; `FASTPQ_METAL_FFT_COLUMNS` (1–32) ni deterministik FFT toʻplam hajmini belgilash uchun va `FASTPQ_METAL_LDE_COLUMNS` (1–32) ni xostlar boʻylab bit-bit taqqoslash kerak boʻlganda LDE dispetcheriga bir xil bekor qilishni qoʻllash uchun oʻrnating. LDE plitka chuqurligi FFT evristikasini ham aks ettiradi - `log₂ ≥ 18/20/22` bilan izlar keng kapalaklarni plitka qo'yishdan keyingi yadroga topshirishdan oldin faqat 12/10/8 umumiy xotira bosqichlarini bajaradi - va siz bu chegarani `FASTPQ_METAL_LDE_TILE_STAGES` (1–3X) orqali bekor qilishingiz mumkin. Ish vaqti barcha qiymatlarni Metall yadro arglari orqali o'tkazadi, qo'llab-quvvatlanmaydigan bekor qilishlarni mahkamlaydi va hal qilingan qiymatlarni qayd qiladi, shuning uchun tajribalar metallibni qayta tiklamasdan takrorlanishi mumkin bo'ladi; Benchmark JSON ham hal qilingan sozlashni, ham LDE statistikasi orqali olingan xostning nol toʻldirish byudjetini (`zero_fill.{bytes,ms,queue_delta}`) koʻrsatadi, shuning uchun navbatdagi deltalar har bir suratga olish uchun toʻgʻridan-toʻgʻri bogʻlanadi va endi `column_staging` blokini qoʻshadi (toʻplamlar tekislangan, flatten_ms, hosters koʻrib chiqishni kutishi mumkin) ikki buferli quvur liniyasi orqali kiritilgan. GPU nol to'ldirish telemetriyasi haqida xabar berishdan bosh tortsa, jabduqlar endi xost tomonidagi buferni tozalashdan deterministik vaqtni sintez qiladi va uni `zero_fill` blokiga kiritadi, shuning uchun dalillarni hech qachon e'lon qilmasdan yubormang. maydon.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. Diskret Mac kompyuterlarida ko‘p navbatli jo‘natish avtomatik amalga oshiriladi: `Device::is_low_power()` noto‘g‘ri javob qaytarsa yoki Metall qurilma Slot/Tashqi joylashuv haqida xabar berganda, xost ikkita `MTLCommandQueue` ni yaratadi, faqat ish yuki ≥16 ustunni ko‘targandan so‘ng ventilyatorlar o‘chadi (va fanatlar qatorlari bo‘ylab miqyosda). uzoq izlar determinizmni buzmasdan ikkala GPU qatorlarini band qiladi. `FASTPQ_METAL_QUEUE_FANOUT` (1–4 navbat) va `FASTPQ_METAL_COLUMN_THRESHOLD` (fan-out oldidan minimal umumiy ustunlar) bilan siyosatni mashinalarda takrorlanadigan suratga olish kerak bo‘lganda bekor qiling; Paritet testlari ushbu bekor qilishni majbur qiladi, shuning uchun ko'p GPUli Maclar yopiq qoladi va hal qilingan fan-out/eshik navbat chuqurligi yonida qayd etiladi. telemetriya.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### Arxiv uchun dalillar
| Artefakt | Rasmga olish | Eslatmalar |
|----------|---------|-------|
| `.metallib` to'plami | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` va `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, keyin `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` va `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Metal CLI/toolchain o'rnatilganligini va bu topshiriq uchun deterministik kutubxona ishlab chiqarilganligini isbotlaydi.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Atrof-muhit lavhasi | Qurilishdan keyin `echo $FASTPQ_METAL_LIB`; chiqish chiptangiz bilan mutlaq yo'lni saqlang. | Bo'sh chiqish Metall o'chirilganligini bildiradi; GPU yoʻllari yuk tashish artefaktida mavjud boʻlib qoladigan qiymat hujjatlarini qayd etish.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU pariteti jurnali | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` va `backend="metal"` yoki pasaytirish ogohlantirishini o'z ichiga olgan parchani arxivlang. | Qurilishni targ'ib qilishdan oldin yadrolar ishlayotganini (yoki deterministik ravishda orqaga ketishini) ko'rsatadi.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Benchmark chiqish | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; o'rash va `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` orqali imzolash. | Oʻralgan JSON yozuvlari `speedup.ratio`, `speedup.delta_ms`, FFT sozlash, toʻldirilgan qatorlar (32.768), boyitilgan `zero_fill`/`kernel_profiles`, tekislangan Prometheus, tasdiqlangan. `metal_dispatch_queue.poseidon`/`poseidon_profiles` bloklari (`--operation poseidon_hash_columns` foydalanilganda) va metama'lumotlarni kuzatish, shuning uchun GPU LDE o'rtacha ≤950ms va Poseidon <1s qoladi; To‘plamni ham, yaratilgan `.json.asc` imzosini ham chiqarish chiptasida saqlang, shunda asboblar paneli va auditorlar artefaktni qayta ishga tushirmasdan tekshirishlari mumkin. ish yuklari.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Bench manifest | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Ikkala GPU artefaktini ham tasdiqlaydi, agar LDE oʻrtacha `<1 s` shiftini buzsa, BLAKE3/SHA-256 dayjestlarini yozsa va imzolangan manifest chiqaradi, shuning uchun chiqarish nazorat roʻyxati tekshirilmasdan oldinga siljiy olmaydi. metrikalar.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA to'plami | SM80 laboratoriya xostida `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`-ni ishga tushiring, JSON-ni `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`-ga o'rang/imzolang (boshqaruv paneli to'g'ri sinfni tanlashi uchun `--label device_class=xeon-rtx-sm80`-dan foydalaning), `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`-ga yo'lni qo'shing va saqlang. `.json`/`.asc` manifestni qayta tiklashdan oldin Metall artefakt bilan birlashtiriladi. Ro'yxatdan o'tgan `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` auditorlar kutayotgan aniq paket formatini ko'rsatadi.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80 |xt:10.
| Telemetriya isboti | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` va ishga tushirish vaqtida chiqarilgan `telemetry::fastpq.execution_mode` jurnali. | Trafikni yoqishdan oldin Prometheus/OTEL `device_class="<matrix>", backend="metal"` (yoki pasaytirish jurnali) chiqishini tasdiqlaydi.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backendrs.:17】 || Majburiy CPU matkap | `FASTPQ_GPU=cpu` yoki `zk.fastpq.execution_mode = "cpu"` bilan qisqa partiyani ishga tushiring va pasaytirish jurnalini yozib oling. | O'rta versiyada orqaga qaytarish kerak bo'lganda, SRE runbook'larini deterministik qayta tiklash yo'liga mos holda ushlab turadi.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Tuzatish (ixtiyoriy) | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` bilan paritet testini takrorlang va chiqarilgan jo'natma izini saqlang. | Qayta ko'rsatkichlarni o'tkazmasdan ko'rib chiqishlarni keyinchalik profillash uchun bandlik/mavzu guruhi dalillarini saqlaydi.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Ko'p tilli `fastpq_plan.*` fayllari ushbu nazorat ro'yxatiga havola qiladi, shuning uchun bosqichma-bosqich va ishlab chiqarish operatorlari bir xil dalillar iziga amal qiladi.【docs/source/fastpq_plan.md:1】

## Qayta tiklanadigan tuzilmalar
Qayta tiklanadigan Stage6 artefaktlarini yaratish uchun mahkamlangan konteyner ish oqimidan foydalaning:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Yordamchi skript `rust:1.88.0-slim-bookworm` asboblar zanjiri tasvirini (va GPU uchun `nvidia/cuda:12.2.2-devel-ubuntu22.04`) yaratadi, konteyner ichida qurishni boshqaradi va maqsadli chiqishga `manifest.json`, `sha256s.txt` va kompilyatsiya qilingan ikkilik fayllarni yozadi. katalog.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Atrof-muhitni bekor qilish:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – aniq Rust bazasi/yorlig'ini mahkamlang.
- `FASTPQ_CUDA_IMAGE` - GPU artefaktlarini ishlab chiqarishda CUDA bazasini almashtiring.
- `FASTPQ_CONTAINER_RUNTIME` - muayyan ish vaqtini majburlash; standart `auto` `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` ni sinab ko'radi.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – ish vaqtini avtomatik aniqlash uchun vergul bilan ajratilgan afzallik tartibi (birlamchi `docker,podman,nerdctl`).

## Konfiguratsiya yangilanishlari
1. TOML da ish vaqtining bajarish rejimini o'rnating:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   Qiymat `FastpqExecutionMode` orqali tahlil qilinadi va ishga tushirilganda backendga uzatiladi.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Agar kerak bo'lsa, ishga tushirish vaqtida bekor qiling:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI bekor qiladi, tugunni ishga tushirishdan oldin hal qilingan konfiguratsiyani o'zgartiradi.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Ishlab chiquvchilar eksport qilish orqali konfiguratsiyalarga tegmasdan aniqlashni vaqtincha majburlashlari mumkin
   Ikkilik faylni ishga tushirishdan oldin `FASTPQ_GPU={auto,cpu,gpu}`; bekor qilish qayd qilinadi va quvur liniyasi
   hali ham hal qilingan rejimni ko'rsatadi.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Tekshirish roʻyxati
1. **Ishga tushirish jurnallari**
   - `telemetry::fastpq.execution_mode` maqsadidan `FASTPQ execution mode resolved`ni kuting.
     `requested`, `resolved` va `backend` teglari.【crates/fastpq_prover/src/backend.rs:208】
   - GPUni avtomatik aniqlashda `fastpq::planner` dan ikkinchi darajali jurnal yakuniy chiziq haqida xabar beradi.
   - Metallib muvaffaqiyatli yuklanganda `backend="metal"` metall xostlar yuzasi; Agar kompilyatsiya yoki yuklash muvaffaqiyatsiz bo'lsa, qurish skripti ogohlantirish chiqaradi, `FASTPQ_METAL_LIB` ni tozalaydi va rejalashtiruvchi davom etishdan oldin `GPU acceleration unavailable` ni yozadi. CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:2. **Prometheus ko'rsatkichlari**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Hisoblagich `record_fastpq_execution_mode` orqali oshiriladi (endi tomonidan belgilangan
   `{device_class,chip_family,gpu_kind}`) tugun o'z bajarilishini hal qilganda
   rejimi.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Metall qoplama uchun tasdiqlang
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     joylashtirish asboblar paneli bilan birga o'sishlar.【crates/iroha_telemetry/src/metrics.rs:5397】
   - `irohad --features fastpq-gpu` bilan tuzilgan macOS tugunlari qo'shimcha ravishda ochiladi
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     va
     Shunday qilib, `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` Stage7 asboblar paneli
     jonli Prometheus tirqishlaridan ish sikli va navbat bo'sh joyini kuzatishi mumkin.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Telemetriya eksporti**
   - OTEL konstruksiyalari bir xil belgilar bilan `fastpq.execution_mode_resolutions_total` chiqaradi; ta'minlang
     asboblar paneli yoki ogohlantirishlar GPU faol bo'lishi kerak bo'lganda kutilmagan `resolved="cpu"` ni kuzatadi.

4. **Aqllilikni isbotlash/tekshirish**
   - `iroha_cli` yoki integratsiya jabduqlar orqali kichik partiyani ishga tushiring va dalillarni tasdiqlang
     peer bir xil parametrlar bilan tuzilgan.

## Nosozliklarni bartaraf etish
- **Resolyutsiya rejimi GPU xostlarida protsessor bo‘lib qoladi** — ikkilik bilan tuzilganligini tekshiring
  `fastpq_prover/fastpq-gpu`, CUDA kutubxonalari yuklash yo'lida va `FASTPQ_GPU` majburlamaydi
  `cpu`.
- **Metal Apple Silicon’da mavjud emas** — CLI asboblari o‘rnatilganligini tekshiring (`xcode-select --install`), `xcodebuild -downloadComponent MetalToolchain`ni qayta ishga tushiring va qurilish bo‘sh bo‘lmagan `FASTPQ_METAL_LIB` yo‘lini hosil qilganiga ishonch hosil qiling; bo'sh yoki etishmayotgan qiymat dizayn bo'yicha backendni o'chirib qo'yadi.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` xatolar** — prover ham, tasdiqlovchi ham bir xil kanonik katalogdan foydalanishiga ishonch hosil qiling
  `fastpq_isi` tomonidan chiqarilgan; `Error::UnknownParameter` sifatida mos kelmaydigan sirt.【crates/fastpq_prover/src/proof.rs:133】
- **Kutilmagan protsessorning ishdan chiqishi** — `cargo tree -p fastpq_prover --features` ni tekshiring va
  GPU tuzilmalarida `fastpq_prover/fastpq-gpu` mavjudligini tasdiqlang; `nvcc`/CUDA kutubxonalari qidiruv yoʻlida ekanligini tekshiring.
- **Telemetriya hisoblagichi yetishmayapti** — tugunning `--features telemetry` bilan boshlanganligini tekshiring (standart)
  va OTEL eksporti (agar yoqilgan bo'lsa) metrik quvur liniyasini o'z ichiga oladi.【crates/iroha_telemetry/src/metrics.rs:8887】

## Qayta tiklash tartibi
Deterministik to'ldiruvchining orqa qismi olib tashlandi. Agar regressiya orqaga qaytishni talab qilsa,
Oldindan ma'lum bo'lgan reliz artefaktlarini qayta joylashtiring va Stage6 ni qayta chiqarishdan oldin tekshirib ko'ring
ikkilik. O'zgarishlarni boshqarish qarorini hujjatlashtiring va faqat o'zgarishlardan so'ng oldinga siljish tugallanishiga ishonch hosil qiling
regressiya tushuniladi.

3. `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` kutilganni aks ettirishi uchun telemetriyani kuzating
   to'ldiruvchining bajarilishi.

## Uskuna bazasi
| Profil | CPU | GPU | Eslatmalar |
| ------- | --- | --- | ----- |
| Malumot (6-bosqich) | AMD EPYC7B12 (32 yadro), 256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000 qatorli sintetik partiyalar ≤1000ms ni bajarishi kerak.【docs/source/fastpq_plan.md:131】 |
| Faqat CPU uchun | ≥32 jismoniy yadro, AVX2 | – | 20000 qator uchun ~0,9–1,2s kuting; determinizm uchun `execution_mode = "cpu"` ni saqlang. |## Regressiya testlari
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU xostlarida)
- Opsiyonel oltin armatura tekshiruvi:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Ushbu nazorat roʻyxatidan har qanday ogʻishlarni oʻzingizning operatsiya kitobingizga hujjatlang va keyin `status.md` yangilang.
migratsiya oynasi tugallanadi.