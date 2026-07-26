---
lang: uz
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2025-12-29T18:16:35.920013+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# solishtirish hisoboti

Har bir ishga tushirish uchun batafsil suratlar va FASTPQ WP5-B tarixi mavjud
[`benchmarks/history.md`](benchmarks/history.md); biriktirganda ushbu indeksdan foydalaning
yo'l xaritasi ko'rib chiqish yoki SRE audit uchun artefaktlar. Uni qayta tiklang
`python3 scripts/fastpq/update_benchmark_history.py` har doim yangi GPU ushlaydi
yoki Poseidon erni namoyon qiladi.

## Tezlashtirish dalillari to'plami

Har bir GPU yoki aralash rejimli benchmark qo'llaniladigan tezlashtirish sozlamalarini o'z ichiga olishi kerak
shuning uchun WP6-B/WP6-C vaqt artefaktlari bilan bir qatorda konfiguratsiya paritetini isbotlashi mumkin.

- Har bir ishga tushirishdan oldin/keyin ish vaqti suratini oling:
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (odam o'qiy oladigan jurnallar uchun `--format table` dan foydalaning). Bu `enable_{metal,cuda}`,
  Merkle chegaralari, SHA-2 protsessor chegaralari, aniqlangan backend sog'liq bitlari va har qanday
  yopishqoq paritet xatolar yoki o'chirish sabablari.
- JSON-ni o'ralgan benchmark chiqishi yonida saqlang
  (`artifacts/fastpq_benchmarks/*.json`, `benchmarks/poseidon/*.json`, Merkle supurgi
  suratga olish va h.k.) shuning uchun sharhlovchilar vaqt va konfiguratsiyani birgalikda farqlashlari mumkin.
- Knob ta'riflari va standart sozlamalari `docs/source/config/acceleration.md` da mavjud; qachon
  bekor qilish qoʻllaniladi (masalan, `ACCEL_MERKLE_MIN_LEAVES_GPU`, `ACCEL_ENABLE_CUDA`),
  Qayta ishlashni xostlar orasida takrorlanishini ta'minlash uchun ularni ishga tushirish metama'lumotlarida qayd qiling.

## Norito 1-bosqich benchmark (WP5-B/C)

- Buyruq: `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  `benchmarks/norito_stage1/` ostida JSON + Markdown chiqaradi, har bir o'lcham vaqtlari bilan
  skaler va tezlashtirilgan strukturaviy indeks yaratuvchisi uchun.
- Oxirgi ishga tushirishlar (macOS aarch64, dev profili).
  `benchmarks/norito_stage1/latest.{json,md}` va yangi kesilgan CSV dan
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) SIMDni ko'rsatadi
  ~6–8KiB dan boshlab yutadi. GPU/parallel Stage-1 endi sukut bo‘yicha **192KiB** ga o‘rnatiladi
  ishga tushirilishining oldini olish uchun kesish (o'chirish uchun `NORITO_STAGE1_GPU_MIN_BYTES=<n>`).
  kichik hujjatlarda katta yuklamalar uchun tezlatgichlarni yoqish.

## Enum va Trait Object Dispatch

- Kompilyatsiya vaqti (disklarni tuzatish): 16.58s
- Ish vaqti (Kriteriya, pastroqsi yaxshiroq):
  - `enum`: 386 ps (o'rtacha)
  - `trait_object`: 1,56 ns (o'rtacha)

Ushbu o'lchovlar raqamga asoslangan jo'natishni qutidagi xususiyat ob'ektini amalga oshirish bilan taqqoslaydigan mikrobenchmarkdan olingan.

## Poseidon CUDA to'plami

Poseidon benchmarki (`crates/ivm/benches/bench_poseidon.rs`) endi bitta xeshli almashtirishlarni va yangi paketli yordamchilarni ishlatadigan ish yuklarini o'z ichiga oladi. To'plamni quyidagilar bilan boshqaring:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion `target/criterion/poseidon*_many` ostida natijalarni qayd qiladi. GPU ishchisi mavjud bo‘lganda, JSON xulosalarini eksport qiling (masalan, `target/criterion/**/new/benchmark.json`ni `benchmarks/poseidon/criterion_poseidon2_many_cuda.json` ga nusxalash) (masalan, `target/criterion/**/new/benchmark.json` ni `benchmarks/poseidon/` ga nusxalash), shuning uchun quyi oqim guruhlari har bir protsessor va CUDA o‘lchamini solishtirishi mumkin. Ajratilgan GPU chizig'i ishga tushgunga qadar, benchmark SIMD/CPU amalga oshirishga qaytadi va hali ham partiya ishlashi uchun foydali regressiya ma'lumotlarini taqdim etadi.

Takrorlanadigan suratga olish uchun (va vaqt ma'lumotlari bilan tenglik dalillarini saqlash uchun) ishga tushiring

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```qaysi urug'lar deterministik Poseidon2/6 partiyalari, CUDA sog'lig'i/o'chirish sabablarini qayd qiladi, tekshiradi
skaler yo'lga nisbatan paritet va Metall bilan birga ops/sek + tezlashtirish xulosalarini chiqaradi
ish vaqti holati (xususiyatlar bayrog'i, mavjudlik, oxirgi xato). Faqat protsessorli xostlar hali ham skalerni yozadilar
mos yozuvlar va etishmayotgan tezlatgichga e'tibor bering, shuning uchun CI artefaktlarni GPUsiz ham nashr qilishi mumkin
yuguruvchi.

## FASTPQ metall mezonlari (Apple Silicon)

GPU qatori macOS 14 (arm64) da `fastpq_metal_bench` ning yangilangan uchdan-end ishini, chiziqli muvozanatlangan parametrlar to‘plami, 20 000 mantiqiy qator (32 768 tagacha to‘ldirilgan) va 16 ta ustun guruhlari bilan suratga oldi. Oʻralgan artefakt `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json` da yashaydi, metall izi `traces/fastpq_metal_trace_*_rows20000_iter5.trace` ostida oldingi suratlar bilan birga saqlanadi. O'rtacha vaqtlar (`benchmarks.operations[*]` dan) endi o'qiydi:

| Operatsiya | CPU o'rtacha (ms) | Metall o'rtacha (ms) | Tezlashtirish (x) |
|----------|---------------|-----------------|-------------|
| FFT (32 768 ta kirish) | 83.29 | 79.95 | 1.04 |
| IFFT (32 768 ta kirish) | 93.90 | 78.61 | 1.20 |
| LDE (262 144 ta kirish) | 669,54 | 657.67 | 1.02 |
| Poseidon xesh ustunlari (524 288 ta kirish) | 29 087,53 | 30 004,90 | 0,97 |

Kuzatishlar:

- FFT/IFFT ikkalasi ham yangilangan BN254 yadrolaridan foyda oladi (IFFT oldingi regressiyani ~20% ga tozalaydi).
- LDE paritetga yaqinligicha qolmoqda; nol-to‘ldirish hozirda o‘rtacha 18,66 ms bilan 33 554 432 to‘ldirilgan baytni yozadi, shuning uchun JSON to‘plami navbat ta’sirini ushlaydi.
- Poseidon xeshlash hali ham ushbu uskunada protsessor bilan bog'langan; Metall yo'l eng so'nggi navbat boshqaruvlarini qabul qilmaguncha, Poseidon mikrobench manifestlari bilan solishtirishda davom eting.
- Har bir qo'lga olish endi `AccelerationSettings.runtimeState().metal.lastError` yozuvlarini beradi, ruxsat beradi
  Muhandislar protsessorning ishdan chiqishiga ma'lum bir o'chirish sababi bilan izoh qo'yishadi (siyosatni o'zgartirish,
  paritet xatosi, qurilma yo'q) to'g'ridan-to'g'ri benchmark artefaktida.

Yugurishni takrorlash uchun Metall yadrolarni yarating va bajaring:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

Olingan JSONni `artifacts/fastpq_benchmarks/` ostida metall izi bilan birga bajaring, shunda determinizm dalillari takrorlanishi mumkin.

## FASTPQ CUDA avtomatizatsiyasi

CUDA xostlari SM80 benchmarkini bir bosqichda ishga tushirishi va oʻrashi mumkin:

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

Yordamchi `fastpq_cuda_bench` ni chaqiradi, teglar/qurilma/qaydlar orqali o'tadi, hurmat qiladi
`--require-gpu` va (sukut bo'yicha) `scripts/fastpq/wrap_benchmark.py` orqali o'raladi/belgilar.
Chiqishlarga xom JSON, `artifacts/fastpq_benchmarks/` ostida oʻralgan toʻplam kiradi,
va `<name>_plan.json`, natijada aniq buyruqlar/env qayd etiladi.
7-bosqich tasvirlari GPU ishlovchilarida takrorlanishi mumkin. `--sign-output` va qo'shing
Imzolar kerak bo'lganda `--gpg-key <id>`; faqat chiqarish uchun `--dry-run` dan foydalaning
dastgohni bajarmasdan rejalashtirish/yo'llar.

### GA relizni suratga olish (macOS 14 arm64, chiziqli balanslangan)

WP2-D ni qondirish uchun biz GA-tayyor bilan bir xil xostda relizlar tuzilishini ham yozdik.
evristikani navbatga qo'ying va uni shunday nashr qildi
`fastpq_metal_bench_20k_release_macos14_arm64.json`. Artefakt ikkitani ushlaydi
ustunlar partiyalari (bo'lakda muvozanatlangan, 32 768 qatorga to'ldirilgan) va Poseidonni o'z ichiga oladi
asboblar panelini iste'mol qilish uchun mikrobench namunalari.| Operatsiya | CPU o'rtacha (ms) | Metall o'rtacha (ms) | Tezlashtirish | Eslatmalar |
|----------|---------------|-----------------|---------|-------|
| FFT (32 768 ta kirish) | 12.741 | 10.963 | 1,16× | GPU yadrolari yangilangan navbat chegaralarini kuzatib boradi. |
| IFFT (32 768 ta kirish) | 17.499 | 25.688 | 0,68× | Konservativ navbat fan-out bilan kuzatilgan regressiya; evristikani sozlashda davom eting. |
| LDE (262 144 ta kirish) | 68.389 | 65.701 | 1,04× | Ikkala to'plam uchun 9,651 ms da 33 554 432 baytni nol to'ldirish jurnallari. |
| Poseidon xesh ustunlari (524 288 ta kirish) | 1,728.835 | 1,447.076 | 1,19× | Poseidon navbatini o'zgartirgandan so'ng, GPU nihoyat protsessorni uradi. |

JSON-ga o'rnatilgan Poseidon mikrobench qiymatlari 1,10 × tezlikni ko'rsatadi (standart chiziq
596,229 ms va beshta iteratsiya bo'yicha skalyar 656,251 ms), shuning uchun asboblar paneli endi diagramma yaratishi mumkin
asosiy skameyka bilan bir qatorda har bir qatorda yaxshilanishlar. Yugurishni quyidagi bilan takrorlang:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

Oʻralgan JSON va `FASTPQ_METAL_TRACE_CHILD=1` izlarini ostida saqlang
`artifacts/fastpq_benchmarks/`, shuning uchun keyingi WP2-D/WP2-E sharhlari GA ni farq qilishi mumkin
ish yukini qayta ishga tushirmasdan oldingi yangilash ishlariga qarshi suratga oling.

Har bir yangi `fastpq_metal_bench` surati endi `bn254_metrics` blokini ham yozadi,
CPU uchun `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` yozuvlarini ochib beradi
asosiy va qaysi GPU backend (Metal/CUDA) faol bo'lsa, **va** a
`bn254_dispatch` bloki, kuzatilgan iplar guruhining kengligini, mantiqiy ipni qayd qiladi
bir ustunli BN254 FFT/LDE jo'natmalari uchun hisoblar va quvur liniyasi chegaralari. The
benchmark wrapper ikkala xaritani `benchmarks.bn254_*` ga ko'chiradi, shuning uchun asboblar paneli va
Prometheus eksportchilari yorliqli kechikishlar va geometriyani qayta tahlil qilmasdan qirib tashlashlari mumkin
xom operatsiyalar massivi. `FASTPQ_METAL_THREADGROUP` bekor qilish endi amal qiladi
BN254 yadrolari, shuningdek, iplar guruhini tozalashni bittadan takrorlanishi mumkin tugma.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_10【

Pastki oqim panellarini oddiy saqlash uchun `python3 scripts/benchmarks/export_csv.py` ni ishga tushiring
bir to'plamni qo'lga olgandan keyin. Yordamchi `poseidon_microbench_*.json` ichiga tekislaydi
mos keladigan `.csv` fayllari, shuning uchun avtomatlashtirish ishlari standart va skaler chiziqlarsiz farq qilishi mumkin
maxsus tahlilchilar.

## Poseidon mikrobench (metall)

`fastpq_metal_bench` endi `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ostida o'zini qayta ishga tushiradi va vaqtni `benchmarks.poseidon_microbench` ga targ'ib qiladi. Biz `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` bilan eng yangi metall suratlarni eksport qildik va ularni `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json` orqali jamladik. Quyidagi xulosalar `benchmarks/poseidon/` ostida ishlaydi:

| Xulosa | Oʻralgan toʻplam | Standart o'rtacha (ms) | Skalar o'rtacha (ms) | Tezlashtirish va skalar | Ustunlar x holatlar | Takrorlashlar |
|---------|----------------|-------------------|------------------|-------------------|------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1 990,49 | 1,994,53 | 1.002 | 64 x 262,144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2,167,66 | 2,152,18 | 0,993 | 64 x 262,144 | 5 |Har ikkala suratga olishda ham bitta isinish iteratsiyasi bilan har bir yugurishda 262 144 holat (trace log2 = 12) xeshlangan. "Standart" qator sozlangan ko'p shtatli yadroga mos keladi, "skalar" esa yadroni taqqoslash uchun har bir chiziq uchun bitta holatga qulflaydi.

## Merkle ostonasini tozalash

`merkle_threshold` misoli (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) Metal-vs-CPU Merkle xeshing yo'llarini ta'kidlaydi. Eng yangi AppleSilicon surati (Darvin 25.0.0 arm64, `ivm::metal_available()=true`) mos keladigan CSV eksporti bilan `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` da saqlanadi. Faqat protsessorli macOS 14 bazasi metallsiz xostlar uchun `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` ostida qoladi.

| Barglar | CPU eng yaxshi (ms) | Metall eng yaxshi (ms) | Tezlashtirish |
|--------|---------------|-----------------|---------|
| 1,024 | 23.01 | 19.69 | 1,17× |
| 4,096 | 50.87 | 62.12 | 0,82× |
| 8,192 | 95.77 | 96.57 | 0,99× |
| 16,384 | 64.48 | 58.98 | 1,09× |
| 32 768 | 109.49 | 87.68 | 1,25× |
| 65,536 | 177,72 | 137.93 | 1,29× |

Kattaroq barglar soni Metalldan foyda ko'radi (1,09-1,29 ×); kichik chelaklar hali ham protsessorda tezroq ishlaydi, shuning uchun CSV ikkala ustunni ham tahlil qilish uchun saqlaydi. CSV yordamchisi GPU va protsessor regressiyasi boshqaruv panelini bir xilda ushlab turish uchun har bir profil yonida `metal_available` bayrog'ini saqlaydi.

Reproduksiya bosqichlari:

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

Agar xost aniq metallni yoqishni talab qilsa, `FASTPQ_METAL_LIB`/`FASTPQ_GPU` ni o‘rnating va WP1-F siyosat chegaralarini belgilashi uchun ikkala CPU + GPU tasvirlarini ham tekshirib turing.

Boshsiz qobiqdan ishlayotganda, `IVM_DEBUG_METAL_ENUM=1` qurilma ro'yxatini jurnalga o'rnating va `IVM_FORCE_METAL_ENUM=1` - `MTLCreateSystemDefaultDevice()` ni chetlab o'ting. CLI standart Metal qurilmani so'rashdan **oldin** CoreGraphics seansini isitadi va `MTLCopyAllDevices()` nolga qaytganida `MTLCreateSystemDefaultDevice()` ga qaytadi; agar xost hali hech qanday qurilma haqida xabar bermasa, yozib olish `metal_available=false` (foydali protsessor bazalari `macos14_arm64_*` ostida ishlaydi), GPU xostlari esa `FASTPQ_GPU=metal` yoqilgan bo'lishi kerak, shunda to'plam tanlangan backendni qayd qiladi.

`fastpq_metal_bench` shunga o'xshash tugmani `FASTPQ_DEBUG_METAL_ENUM=1` orqali ochib beradi, u `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` natijalarini orqa tomon GPU yo'lida qolish-qolmasligini hal qilishdan oldin chop etadi. `FASTPQ_GPU=gpu` oʻralgan JSONda hali ham `backend="none"` haqida xabar berganda uni yoqing, shuning uchun suratga olish toʻplami xostning Metall qurilmalarni qanday sanaganini aniq qayd etadi; jabduqlar `FASTPQ_GPU=gpu` o'rnatilganda darhol to'xtatiladi, lekin hech qanday tezlatgich aniqlanmasa, disk raskadrovka tugmasiga ishora qiladi, shuning uchun chiqarish to'plami hech qachon majburiy GPU orqasida CPU zaxirasini yashirmaydi. ishga tushirish.【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】

CSV yordamchisi har bir profil uchun jadvallarni chiqaradi (masalan, `macos14_arm64_*.csv` va `takemiyacStudio.lan_25.0.0_arm64.csv`), `metal_available` bayrog'ini saqlab qoladi, shuning uchun regressiya asboblar paneli protsessor va GPU o'lchovlarini maxsus tahlilchilarsiz qabul qilishi mumkin.