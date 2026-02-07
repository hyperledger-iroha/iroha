---
lang: uz
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2025-12-29T18:16:35.976997+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Metall va NEON tezlashtirish rejasi (Swift & Rust)

Ushbu hujjat deterministik uskunani yoqish uchun umumiy rejani qamrab oladi
tezlashtirish (Metal GPU + NEON / Accelerate SIMD + StrongBox integratsiyasi)
Rust ish maydoni va Swift SDK. U kuzatilgan yo'l xaritasi elementlariga murojaat qiladi
**Hardware Acceleration Workstream (macOS/iOS)** ostida va uzatishni ta'minlaydi
Rust IVM jamoasi, Swift ko'prigi egalari va telemetriya asboblari uchun artefakt.

> Oxirgi yangilangan: 2026-01-12  
> Egalari: IVM Performance TL, Swift SDK Lead

## Maqsadlar

1. Metal orqali Apple apparatida Rust GPU yadrolarini (Poseidon/BN254/CRC64) qayta ishlating
   CPU yo'llariga nisbatan deterministik paritetga ega hisoblash shaderlari.
2. Tezlashtirish oʻtish tugmalarini (`AccelerationConfig`) uchdan uchiga oching, shunda Swift ilovalari
   ABI/paritet kafolatlarini saqlab qolgan holda Metal/NEON/StrongBox-ni tanlashi mumkin.
3. Parite/benchmark ma'lumotlari va bayroqni yuzaga chiqarish uchun CI + asboblar panelidan foydalaning
   CPU va GPU/SIMD yo'llari bo'ylab regressiya.
4. Android (AND2) va Swift o'rtasida StrongBox/xavfsiz anklav darslarini baham ko'ring
   (IOS4) imzolash oqimlarini qat'iy ravishda moslashtirish uchun.

**Yangilanish (CRC64 + Stage‑1 yangilanishi):** CRC64 GPU yordamchilari endi 192KiB standart uzilish bilan `norito::core::hardware_crc64` ga ulangan (`NORITO_GPU_CRC64_MIN_BYTES` yoki aniq yordamchi yoʻl Prometheus orqali bekor qilinadi). JSON Stage‑1 kesishmalari qayta sinovdan o‘tkazildi (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`), skalyar kesish 4KiB da saqlangan va Stage‑1 GPU sukut bo‘yicha 192KiB ga tenglashtirildi (`NORITO_STAGE1_GPU_MIN_BYTES` uchun GPU uchun katta yuk bo‘lgan hujjatlarni ishga tushirish va to‘lovli bo‘lishi mumkin).

## Etkazib berish va egalari

| Muhim bosqich | Yetkazib beriladi | Ega(lar)i | Maqsad |
|----------|-------------|----------|--------|
| Rust WP2-A/B | CUDA yadrolarini aks ettiruvchi metall shader interfeyslari | IVM Perf TL | 2026 yil fevral |
| Rust WP2-C | Metall BN254 paritet testlari & CI qator | IVM Perf TL | 2026 yil 2-chorak |
| Swift IOS6 | Ko'prik simli (`connect_norito_set_acceleration_config`) + SDK API + namunalari | Swift Bridge egalari | Bajarildi (2026 yil yanvar) |
| Swift IOS5 | Konfiguratsiyadan foydalanishni ko'rsatadigan namunaviy ilovalar/hujjatlar | Swift DX TL | 2026 yil 2-chorak |
| Telemetriya | Boshqaruv paneli tasmasi tezlashuv pariteti + benchmark koʻrsatkichlari | Swift dasturi PM / Telemetriya | 2026 yil 2-chorak pilot ma'lumotlari |
| CI | XCFramework tutun jabduqlari protsessor va metall/NEON o'rtasida qurilma hovuzida | Swift QA rahbari | 2026 yil 2-chorak |
| StrongBox | Uskuna bilan ta'minlangan imzo pariteti testlari (birgalikda vektorlar) | Android Crypto TL / Swift Security | 2026 yil 3-chorak |

## Interfeyslar va API shartnomalari### Zang (`ivm::AccelerationConfig`)
- Mavjud maydonlarni saqlang (`enable_simd`, `enable_metal`, `enable_cuda`, `max_gpus`, chegaralar).
- Birinchi foydalanish kechikishining oldini olish uchun aniq metall isitishni qo'shing (Rust #15875).
- Boshqaruv panellari holatini/diagnostikasini qaytaruvchi paritet API-larni taqdim eting:
  - masalan. `ivm::vector::metal_status()` -> {yoqilgan, paritet, oxirgi_xato}.
- Chiqishni taqqoslash ko'rsatkichlari (Merkle daraxt vaqtlari, CRC o'tkazuvchanligi) orqali
  `ci/xcode-swift-parity` uchun telemetriya ilgaklari.
- Metall xost endi kompilyatsiya qilingan `fastpq.metallib` ni yuklaydi, FFT/IFFT/LDE-ni yuboradi
  va Poseidon yadrolari va har doim protsessorni amalga oshirishga qaytadi
  metallib yoki qurilma navbati mavjud emas.

### C FFI (`connect_norito_bridge`)
- Yangi tuzilma `connect_norito_acceleration_config` (tugallangan).
- Getter qamroviga endi sozlagichni aks ettirish uchun `connect_norito_get_acceleration_config` (faqat konfiguratsiya) va `connect_norito_get_acceleration_state` (konfiguratsiya + paritet) kiradi.
- SPM/CocoaPods iste'molchilari uchun sarlavha izohlarida hujjat tuzilishi tartibi.

### Swift (`AccelerationSettings`)
- Standart sozlamalar: Metall yoqilgan, CUDA o'chirilgan, chegaralar nolga teng (meros).
- salbiy qiymatlar e'tiborga olinmaydi; `apply()` `IrohaSDK` tomonidan avtomatik ravishda chaqiriladi.
- `AccelerationSettings.runtimeState()` endi `connect_norito_get_acceleration_state` ni chiqaradi
  foydali yuk (konfiguratsiya + Metal/CUDA pariteti holati), shuning uchun Swift asboblar paneli bir xil telemetriyani chiqaradi
  Rust sifatida (`supported/configured/available/parity`). Yordamchi `nil` ni qaytarganda
  Sinovlarni ko'chma saqlash uchun ko'prik yo'q.
- `AccelerationBackendStatus.lastError` dan o'chirish/xato sababini ko'chiradi
  `connect_norito_get_acceleration_state` va satr tugashi bilan mahalliy buferni bo'shatadi
  Mobil paritet asboblar paneli nima uchun Metal/CUDA o'chirilganligini izohlashi mumkin bo'lgan tarzda amalga oshirildi
  har bir uy egasi.
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  hozir `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift` ostida testlar
  operator manifestlarini Norito demosi bilan bir xil ustuvor tartibda hal qiladi: sharaf
  `NORITO_ACCEL_CONFIG_PATH`, qidirish to'plami `acceleration.{json,toml}` / `client.{json,toml}`,
  tanlangan manbaga kiring va standart sozlamalarga qayting. Ilovalar endi maxsus yuklagichlarga muhtoj emas
  Rust `iroha_config` sirtini aks ettiring.
- Oʻzgartirish va telemetriya integratsiyasini koʻrsatish uchun namuna ilovalari va README ni yangilang.

### Telemetriya (boshqarish paneli + eksportchilar)
- Paritet tasmasi (mobile_parity.json):
  - `acceleration.metal/neon/strongbox` -> {yoqilgan, paritet, perf_delta_pct}.
  - `perf_delta_pct` asosiy protsessor va GPU taqqoslashini qabul qiling.
  - `acceleration.metal.disable_reason` nometall `AccelerationBackendStatus.lastError`
    shuning uchun Swift avtomatizatsiyasi o'chirilgan GPU-larni Rust bilan bir xil aniqlik bilan belgilashi mumkin
    asboblar paneli.
- CI tasmasi (mobile_ci.json):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {cpu, metall}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> Ikki marta.
- Eksportchilar ko'rsatkichlarni Rust benchmarklaridan yoki CI ishlaridan (masalan, run
  `ci/xcode-swift-parity` qismi sifatida metall/CPU mikrobench).### Konfiguratsiya tugmalari va standart sozlamalar (WP6-C)
- `AccelerationConfig` sukut bo'yicha: macOS qurilmalarida `enable_metal = true`, CUDA funksiyasi kompilyatsiya qilinganda `enable_cuda = true`, `max_gpus = None` (qopqoqsiz). Swift `AccelerationSettings` o'rami `connect_norito_set_acceleration_config` orqali bir xil standart sozlamalarni meros qilib oladi.
- Norito Merkle evristikasi (GPU va protsessorga qarshi): `merkle_min_leaves_gpu = 8192` ≥8192 barglari bo'lgan daraxtlar uchun GPU xeshlash imkonini beradi; backend bekor qilishlari (`merkle_min_leaves_metal`, `merkle_min_leaves_cuda`), agar aniq belgilanmagan bo'lsa, sukut bo'yicha bir xil chegaraga.
- CPU afzal evristikasi (SHA2 ISA mavjud): ikkala AArch64 (ARMv8 SHA2) va x86/x86_64 (SHA-NI) da protsessor yo'li `prefer_cpu_sha2_max_leaves_* = 32_768` tark etgunga qadar afzalroq bo'lib qoladi; GPU chegarasi amal qiladi. Bu qiymatlar `AccelerationConfig` orqali sozlanishi mumkin va faqat benchmark dalillari bilan sozlanishi kerak.

## Sinov strategiyasi

1. **Birlik pariteti testlari (Rust)**: metall yadrolari protsessor chiqishiga mos kelishiga ishonch hosil qiling.
   deterministik vektorlar; `cargo test -p ivm --features metal` ostida ishlaydi.
   `crates/fastpq_prover/src/metal.rs` endi faqat macOS uchun paritet testlarini yuboradi
   skaler mos yozuvlar qarshi FFT/IFFT/LDE va Poseidon mashq qiling.
2. **Swift tutun jabduq**: CPU va Metallni bajarish uchun IOS6 sinov qurilmasini kengaytiring
   ikkala emulyatorda ham, StrongBox qurilmalarida ham kodlash (Merkle/CRC64); solishtiring
   natijalar va jurnal pariteti holati.
3. **CI**: bosish uchun `norito_bridge_ios.yml` ni yangilang (allaqachon `make swift-ci` chaqiradi)
   artefaktlarga tezlashtirish ko'rsatkichlari; ishga tushirish Buildkite-ni tasdiqlashiga ishonch hosil qiling
   `ci/xcframework-smoke:<lane>:device_tag` metadata jabduqlar o'zgarishlarini e'lon qilishdan oldin,
   va parite/benchmark drift bo'yicha chiziqdan o'tmang.
4. **Boshqaruv paneli**: yangi maydonlar endi CLI chiqishida ko'rsatiladi. Eksportchilar ishlab chiqarishni ta'minlash
   asboblar panelini jonli ravishda aylantirishdan oldin ma'lumotlar.

## WP2-A Metall shader rejasi (Poseidon quvurlari)

Birinchi WP2 bosqichi Poseidon Metal yadrolari uchun rejalashtirish ishlarini qamrab oladi
bu CUDA dasturini aks ettiradi. Reja harakatlarni yadrolarga ajratadi,
Xostni rejalashtirish va umumiy doimiy sahnalashtirish, shuning uchun keyingi ishlar faqat diqqatni qaratishi mumkin
amalga oshirish va sinovdan o'tkazish.

### Yadro doirasi

1. `poseidon_permute`: `state_count` mustaqil davlatlarni almashtiradi. Har bir ip
   `STATE_CHUNK` (4 shtat) ga ega va barcha `TOTAL_ROUNDS` iteratsiyalarini ishlatadi.
   jo'natish vaqtida bosqichli ish zarralari guruhi bilan birgalikda dumaloq konstantalar.
2. `poseidon_hash_columns`: siyrak `PoseidonColumnSlice` katalogini o'qiydi va
   Har bir ustunning Merkle-do'st xeshlashni amalga oshiradi (protsessorga mos keladi
   `PoseidonColumnBatch` tartibi). U bir xil ish zarralari guruhi doimiy buferidan foydalanadi
   permute yadrosi sifatida, lekin `(states_per_lane * block_count)` ustidan tsikllar
   Yadro navbatdagi yuborishlarni amortizatsiya qilishi uchun chiqadi.
3. `poseidon_trace_fused`: kuzatuv jadvali uchun ota/barg digestlarini hisoblaydi
   bitta o'tishda. Birlashtirilgan yadro `PoseidonFusedArgs` ni iste'mol qiladi, shuning uchun xost
   qo'shni bo'lmagan hududlarni va `leaf_offset`/`parent_offset` va
   u barcha dumaloq/MDS jadvallarini boshqa yadrolar bilan baham ko'radi.

### Buyruqlarni rejalashtirish va xost shartnomalari- Har bir yadro yuborilishi `MetalPipelines::command_queue` orqali amalga oshiriladi, bu
  adaptiv rejalashtiruvchini (maqsad ~2 ms) va navbatni chiqarishni boshqarishni amalga oshiradi
  `FASTPQ_METAL_QUEUE_FANOUT` orqali ta'sir va
  `FASTPQ_METAL_COLUMN_THRESHOLD`. `with_metal_state` da isitish yo'li
  Barcha uchta Poseidon yadrosini oldindan kompilyatsiya qiladi, shuning uchun birinchi jo'natma buni qilmaydi
  quvur liniyasini yaratish uchun jarima to'lash.
- Mavzular guruhining o'lchamlari mavjud metall FFT/LDE standartlarini aks ettiradi: maqsad
  Har bir topshiriq uchun 8192 ta mavzu, har bir guruh uchun 256 ta mavzu. The
  xost kam quvvatli qurilmalar uchun `states_per_lane` multiplikatorini pastga tushirishi mumkin
  atrof-muhitni terish bekor qiladi (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  WP2-B da qo'shiladi) shader mantig'ini o'zgartirmasdan.
- Ustunlarni joylashtirish FFT tomonidan allaqachon qo'llanilgan bir xil ikki buferli hovuzga amal qiladi
  quvurlar. Poseidon yadrolari ushbu bosqich buferiga xom ko'rsatkichlarni qabul qiladi
  va hech qachon global yig'ma taqsimotlarga tegmang, bu xotira determinizmini saqlaydi
  CUDA xostiga moslashtirilgan.

### Umumiy doimiylar

- `PoseidonSnapshot` manifestida tasvirlangan
  `docs/source/fastpq/poseidon_metal_shared_constants.md` endi kanonik hisoblanadi
  dumaloq konstantalar va MDS matritsasi uchun manba. Ikkala metall (`poseidon2.metal`)
  va CUDA (`fastpq_cuda.cu`) yadrolari manifest bo'lganda qayta tiklanishi kerak.
  o'zgarishlar.
- WP2-B ish vaqtida manifestni o'qiydigan kichik xost yuklagichini qo'shadi va
  SHA-256 ni telemetriyaga chiqaradi (`acceleration.poseidon_constants_sha`).
  Parite panellari shader konstantalari nashr etilganlarga mos kelishini ta'kidlashi mumkin
  surat.
- Isitish paytida biz `TOTAL_ROUNDS x STATE_WIDTH` konstantalarini a ga ko'chiramiz
  `MTLBuffer` va uni har bir qurilmaga bir marta yuklang. Keyin har bir yadro ma'lumotlarni nusxalaydi
  uning bo'lagini qayta ishlashdan oldin ish zarralari guruhi xotirasiga kiritib, deterministikni ta'minlaydi
  bir nechta buyruqlar buferlari parvozda ishlaganda ham buyurtma berish.

### Tasdiqlash ilgaklari

- Birlik testlari (`cargo test -p fastpq_prover --features fastpq-gpu`) o'sadi
  o'rnatilgan shader konstantalarini xeshlash va ularni solishtirish
  GPU moslamalar to'plamini ishga tushirishdan oldin manifestning SHA.
- Mavjud yadro statistikasi o'zgaradi (`FASTPQ_METAL_TRACE_DISPATCH`,
  `FASTPQ_METAL_QUEUE_FANOUT`, navbat chuqurligi telemetriyasi) talab qilinadigan dalilga aylanadi
  WP2-dan chiqish uchun: har bir sinov ishini rejalashtiruvchi hech qachon buzilmasligini isbotlashi kerak
  sozlangan fan-out va birlashtirilgan iz yadrosi navbatni pastda ushlab turadi
  moslashuvchan oyna.
- Swift XCFramework tutun jabduqlari va Rust benchmark yuguruvchilari boshlanadi
  `acceleration.poseidon.permute_p90_ms{cpu,metal}` eksport qilmoqda, shuning uchun WP2-D diagrammaga ega bo'lishi mumkin
  Metall va protsessor deltalari yangi telemetriya tasmalarini qayta kashf qilmasdan.

## WP2-B Poseidon Manifest Loader & O'z-o'zini sinov pariteti- `fastpq_prover::poseidon_manifest()` endi joylashtiradi va tahlil qiladi
  `artifacts/offline_poseidon/constants.ron`, SHA-256 ni hisoblaydi
  (`poseidon_manifest_sha256()`) va protsessorga nisbatan suratni tasdiqlaydi
  har qanday GPU ishi boshlanishidan oldin poseidon jadvallari. `build_metal_context()` qayd qiladi
  Telemetriya eksportchilari nashr qilishlari uchun qizdirish paytida hazm qilish
  `acceleration.poseidon_constants_sha`.
- Manifest parser mos kelmaydigan kenglik/stavka/dumaloq hisoblash kortejlarini va rad etadi
  manifest MDS matritsasi skalyar amalga oshirishga tengligini ta'minlaydi, oldini oladi
  kanonik jadvallar qayta tiklanganda jim drift.
- `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs` qo'shildi, qaysi
  `poseidon2.metal` ichiga o'rnatilgan Poseidon jadvallarini tahlil qiladi va
  `fastpq_cuda.cu` va ikkala yadro aynan bir xil seriyali ekanligini tasdiqlaydi
  manifest sifatida konstantalar. Agar kimdir shader/CUDA ni tahrir qilsa, CI endi ishlamay qoladi
  kanonik manifestni qayta tiklamasdan fayllar.
- Kelajakdagi paritet ilgaklari (WP2-C/D) `poseidon_manifest()` dan qayta foydalanishi mumkin.
  konstantalarni GPU buferlariga aylantiring va Norito orqali dayjestni oching
  telemetriya tasmasi.

## WP2-C BN254 Metall quvurlar va paritet sinovlari- **Qo'l va bo'shliq:** Xost dispetcherlari, paritet jabduqlar va `bn254_status()` jonli va `crates/fastpq_prover/metal/kernels/bn254.metal` endi Montgomery primitivlarini hamda tarmoqli-sinxronlashtirilgan FFT/LDE halqalarini qo'llaydi. Har bir jo'natma bosqichli to'siqlarga ega bo'lgan bitta ish zarrachasi ichida butun ustunni boshqaradi, shuning uchun yadrolar bosqichli manifestlarni parallel ravishda amalga oshiradi. Telemetriya endi simga ulangan va rejalashtiruvchining bekor qilinishi sharaflangan, shuning uchun biz Goldilocks yadrolari uchun ishlatadigan dalillar bilan sukut bo'yicha ishga tushirishni amalga oshirishimiz mumkin.
- **Yadro talablari:** ✅ bosqichli aylanma/koset manifestlarini qayta ishlating, kirish/chiqishlarni bir marta oʻzgartiring va barcha radix-2 bosqichlarini har bir ustunli tarmoq guruhi ichida bajaring, shunda bizga koʻp dispetcherlik sinxronizatsiyasi kerak boʻlmaydi. Montgomery yordamchilari FFT/LDE o'rtasida umumiy bo'lib qoladi, shuning uchun faqat halqa geometriyasi o'zgardi.
- **Xost simlari:** ✅ `crates/fastpq_prover/src/metal.rs` kanonik a'zolarni bosqichma-bosqich amalga oshiradi, LDE buferini nol bilan to'ldiradi, har bir ustun uchun bitta ip guruhini tanlaydi va `bn254_status()` ni o'tish uchun ochadi. Telemetriya uchun qo'shimcha xostni o'zgartirish talab qilinmaydi.
- **Qo'riqchilarni yarating:** `fastpq.metallib` plitkali yadrolarni jo'natadi, shuning uchun shader siljishida CI hali ham tez ishlamay qoladi. Kelajakdagi har qanday optimallashtirish kompilyatsiya vaqti kalitlari o'rniga telemetriya/xususiyatlar eshiklari ortida qoladi.
- **Paritet moslamalari:** ✅ `bn254_parity` testlari GPU FFT/LDE chiqishlarini protsessor qurilmalari bilan solishtirishda davom etmoqda va endi Metall uskunada jonli ishlaydi; yangi yadro kod yo'llari paydo bo'lsa, buzilgan manifest testlarini yodda tuting.
- **Telemetriya va sinovlar:** `fastpq_metal_bench` endi chiqaradi:
  - `bn254_dispatch` bloki FFT/LDE bir ustunli partiyalar uchun har bir jo'natmali iplar guruhining kengligi, mantiqiy iplar soni va quvur liniyasi chegaralarini umumlashtiradi; va
  - `bn254_metrics` bloki, u `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` ni protsessorning asosiy chizig'i va qaysi GPU orqa tomoni ishlaganidan qat'iy nazar qayd qiladi.
  Benchmark o'rami ikkala xaritani har bir o'ralgan artefaktga nusxa ko'chiradi, shuning uchun WP2-D asboblar paneli xom operatsiyalar qatorini teskari muhandisliksiz yorliqli kechikishlar/geometriyani qabul qiladi. `FASTPQ_METAL_THREADGROUP` endi BN254 FFT/LDE jo'natmalariga ham taalluqli bo'lib, tugmachani mukammal foydalanish uchun qulay qiladi. supurib.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_10【

## Ochiq savollar (2027 yil may oyida hal qilingan)1. **Metal resurslarini tozalash:** `warm_up_metal()` mahalliy ipni qayta ishlatadi
   `OnceCell` va endi idempotentlik/regressiya testlari mavjud
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`), shuning uchun ilovaning hayot aylanishiga o'tish
   oqish yoki ikki marta ishga tushirishsiz isitish yo'lini xavfsiz chaqirishi mumkin.
2. **Benchmark bazalari:** Metall chiziqlar protsessorning 20% ichida qolishi kerak
   FFT/IFFT/LDE uchun bazaviy va Poseidon CRC/Merkle yordamchilari uchun 15% doirasida;
   ogohlantirish `acceleration.*_perf_delta_pct > 0.20` (yoki etishmayotgan) qachon yonishi kerak
   mobil paritet tasmasida. 20k iz to'plamida kuzatilgan IFFT regressiyalari
   Endi ular WP2-D da qayd etilgan navbatni bekor qilish bilan bog'langan.
3. **StrongBox zaxirasi:** Swift Android-ning zaxira o'yin kitobini kuzatib boradi
   attestatsiyadagi xatolarni qo'llab-quvvatlash kitobida qayd etish
   (`docs/source/sdk/swift/support_playbook.md`) va avtomatik ravishda o'tish
   audit jurnali bilan hujjatlashtirilgan HKDF tomonidan qo'llab-quvvatlanadigan dasturiy ta'minot yo'li; paritet vektorlari
   mavjud OA qurilmalari orqali baham ko'ring.
4. **Telemetriya xotirasi:** Tezlashtirishni suratga olish va qurilma pulini tekshirish
   `configs/swift/` ostida arxivlangan (masalan,
   `configs/swift/xcframework_device_pool_snapshot.json`) va eksportchilar
   bir xil tartibni aks ettirishi kerak (`artifacts/swift/telemetry/acceleration/*.json`
   yoki `.prom`) shuning uchun Buildkite izohlari va portal asboblar paneli
   maxsus qirib tashlamasdan ozuqa beradi.

## Keyingi qadamlar (2026 yil fevral)

- [x] Rust: quruqlikdagi metall xost integratsiyasi (`crates/fastpq_prover/src/metal.rs`) va
      Swift uchun yadro interfeysini ochish; doc hand-off bilan birga kuzatilgan
      Swift bridge eslatma.
- [x] Swift: SDK darajasidagi tezlashtirish sozlamalarini oching (2026 yil yanvarda bajarilgan).
- [x] Telemetriya: `scripts/acceleration/export_prometheus.py` endi konvertatsiya qiladi
      `cargo xtask acceleration-state --format json` chiqishi Prometheus ga
      matn fayli (ixtiyoriy `--instance` yorlig'i bilan), shuning uchun CI dasturlari GPU/CPU biriktirishi mumkin
      to'g'ridan-to'g'ri matn fayliga yoqish, chegaralar va parite/o'chirish sabablari
      buyurtma qirib tashlamasdan kollektorlar.
- [x] Swift QA: `scripts/acceleration/acceleration_matrix.py` bir nechta to'playdi
      qurilma tomonidan kalitlangan JSON yoki Markdown jadvallariga tezlashtirish holatini yozib olish
      yorlig'i, tutun qurilmasiga deterministik "CPU vs Metal/CUDA" matritsasini beradi
      namuna-ilova sigaretlari bilan birga yuklash uchun. Markdown chiqishi aks ettiradi
      Buildkite dalillar formati, shuning uchun asboblar paneli bir xil artefaktni qabul qilishi mumkin.
- [x] status.md ni endi `irohad` navbat/nol-to'ldirish eksportchilari yuboradi va yangilang
      env/config tekshirish testlari Metall navbatni bekor qilishni qamrab oladi, shuning uchun WP2-D
      telemetriya + bog'lashlarga jonli dalillar biriktirilgan.【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

Telemetriya/eksport yordamchi buyruqlari:

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## WP2-D Release Benchmark & Binding Notes- **20k qatorli relizlar suratga olish:** macOS14 da yangi Metal va CPU mezonlari qayd etildi
  (arm64, chiziqli muvozanatlangan parametrlar, to'ldirilgan 32,768 qatorli iz, ikkita ustunli partiyalar) va
  JSON to'plamini `fastpq_metal_bench_20k_release_macos14_arm64.json` ga tekshirdi.
  Benchmark har bir operatsiya vaqtini eksport qiladi va Poseidon mikrobenchini tasdiqlaydi
  WP2-D yangi Metall navbat evristikasiga bog'langan GA sifatli artefaktga ega. Sarlavha
  deltalar (to'liq jadval `docs/source/benchmarks.md` da yashaydi):

  | Operatsiya | CPU o'rtacha (ms) | Metall o'rtacha (ms) | Tezlashtirish |
  |----------|---------------|-----------------|---------|
  | FFT (32 768 ta kirish) | 12.741 | 10.963 | 1,16× |
  | IFFT (32 768 ta kirish) | 17.499 | 25.688 | 0,68× *(regressiya: determinizmni saqlab qolish uchun navbatdagi fan chiqishi bosildi; keyingi sozlash kerak)* |
  | LDE (262 144 ta kirish) | 68.389 | 65.701 | 1,04× |
  | Poseidon xesh ustunlari (524 288 ta kirish) | 1,728.835 | 1,447.076 | 1,19× |

  Har bir suratga olish jurnali `zero_fill` vaqtlarini (33,554,432 bayt uchun 9,651 ms) va
  `poseidon_microbench` yozuvlari (standart qator 596,229 ms va skaler 656,251 ms,
  1,10 × tezlashtirish) shuning uchun asboblar paneli iste'molchilari navbatdagi bosimni farq qilishi mumkin
  asosiy operatsiyalar.
- **Bindings/docs cross-link:** `docs/source/benchmarks.md` endi havola qiladi
  JSON va reproduktor buyrug'ini chiqaring, Metall navbatni bekor qilish tasdiqlanadi
  `iroha_config` env/manifest testlari orqali va `irohad` jonli ravishda nashr etadi
  `fastpq_metal_queue_*` o'lchaydi, shuning uchun asboblar paneli IFFT regressiyalarini belgilamaydi.
  ad-hoc log qirqish. Swiftning `AccelerationSettings.runtimeState` ni ochib beradi
  bir xil telemetriya foydali yuki JSON to'plamida yuborilib, WP2-D ni yopadi
  Qayta tiklanadigan qabul qilish bazasi bilan bog'lovchi/doc bo'shlig'i.【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **IFFT navbatini tuzatish:** Teskari FFT to‘plamlari endi istalgan vaqtda ko‘p navbatli jo‘natishni o‘tkazib yuboradi.
  ish yuki ventilyatsiya chegarasiga zo'rg'a javob beradi (bo'lakda 16 ta ustun
  profili), ushlab turganda yuqorida aytilgan metall-protsessor regressiyasini olib tashlash
  FFT/LDE/Poseidon uchun ko'p navbatli yo'lda katta ustunli ish yuklari.