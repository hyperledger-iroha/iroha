---
lang: uz
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Tezlashtirish va Norito evristik ma'lumotnoma

`[accel]` bloki `iroha_config` orqali o'tadi.
`crates/irohad/src/main.rs:1895` `ivm::set_acceleration_config` ga. Har bir uy egasi
VM ni yaratishdan oldin bir xil tugmalarni qo'llaydi, shuning uchun operatorlar aniqlay oladilar
skaler/SIMD zaxiralarini mavjud bo'lgan holda qaysi GPU backendlariga ruxsat berilganligini tanlang.
Swift, Android va Python ulanishlari bir xil manifestni ko'prik qatlami orqali yuklaydi
ushbu standart sozlamalarni hujjatlash WP6-C-ni apparat tezlashuvi orqasida blokdan chiqaradi.

### `accel` (apparat tezlashuvi)

Quyidagi jadvalda `docs/source/references/peer.template.toml` va
`iroha_config::parameters::user::Acceleration` ta'rifi, atrof-muhitni fosh qilish
har bir kalitni bekor qiladigan o'zgaruvchi.

| Kalit | Env var | Standart | Tavsif |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX ijrosini yoqadi. `false` qachon, VM deterministik paritetni yozib olishni osonlashtirish uchun vektor operatsiyalari va Merkle xeshing uchun skaler orqa uchlarini majburlaydi. |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | U kompilyatsiya qilinganda va ish vaqti barcha oltin vektor tekshiruvlaridan o'tganda CUDA backendini yoqadi. |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | MacOS tuzilmalarida Metall backendni yoqadi. To'g'ri bo'lsa ham, metallning o'z-o'zini sinab ko'rishi, agar paritet mos kelmasligi yuzaga kelsa, ish vaqtida backendni o'chirib qo'yishi mumkin. |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (avtomatik) | Ish vaqti qancha jismoniy GPU ishga tushirilishini belgilaydi. `0` "mos apparat fan-out" degan ma'noni anglatadi va `GpuManager` tomonidan mahkamlanadi. |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Merkle bargini GPUga tushirishdan oldin minimal barglar talab qilinadi. Ushbu chegaradan past qiymatlar PCIe (`crates/ivm/src/byte_merkle_tree.rs:49`) ning oldini olish uchun protsessorda xeshingni davom ettiradi. |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (global meros) | GPU chegarasi uchun metallga xos bekor qilish. Qachon `0`, Metal `merkle_min_leaves_gpu` ni meros qilib oladi. |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (global meros qilib olish) | GPU chegarasi uchun CUDA-ga xos bekor qilish. `0` qachon, CUDA `merkle_min_leaves_gpu` ni meros qilib oladi. |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (ichki 32768) | ARMv8 SHA-2 ko'rsatmalari GPU xeshingdan ustun bo'lishi kerak bo'lgan daraxt o'lchamini ko'rsatadi. `0` `32_768` barglarining kompilyatsiya qilingan sukutini saqlaydi (`crates/ivm/src/byte_merkle_tree.rs:59`). |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (ichki 32768) | Yuqoridagi kabi, lekin SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) dan foydalanadigan x86/x86_64 xostlar uchun. |

`enable_simd` shuningdek, RS16 oʻchirish kodlashni ham boshqaradi (Torii DA ingest + asboblar). Uni o'chirib qo'ying
apparat bo'ylab deterministik natijalarni saqlab, skalyar paritet yaratishni majburlash.

Konfiguratsiyaga misol:

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

Oxirgi beshta kalit uchun nol qiymatlar "kompilyatsiya qilingan standartni saqlash" degan ma'noni anglatadi. Xostlar bunday qilmasligi kerak
ziddiyatli bekor qilishni o'rnatish (masalan, faqat CUDA chegaralarini majburlash paytida CUDA-ni o'chirib qo'yish),
aks holda so'rov e'tiborga olinmaydi va backend global siyosatga amal qilishda davom etadi.

### Ish vaqti holati tekshirilmoqdaIlovani suratga olish uchun `cargo xtask acceleration-state [--format table|json]` ni ishga tushiring
Metal/CUDA ish vaqti sog'lig'i bitlari bilan birga konfiguratsiya. Buyruq ni tortadi
joriy `ivm::acceleration_config`, paritet holati va yopishqoq xato satrlari (agar
backend o'chirilgan) shuning uchun operatsiyalar natijani to'g'ridan-to'g'ri paritetga berishi mumkin
asboblar paneli yoki voqea sharhlari.

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
```

Suratni avtomatlashtirish (JSON) orqali olish kerak bo'lganda `--format json` dan foydalaning
jadvalda ko'rsatilgan bir xil maydonlarni o'z ichiga oladi).

`acceleration_runtime_errors()` endi SIMD nima uchun skalyarga qaytganini aytadi:
`disabled by config`, `forced scalar override`, `simd unsupported on hardware` yoki
`simd unavailable at runtime`, aniqlash muvaffaqiyatli bo'lsa, lekin ijro hali ham ishlaydi
vektorlarsiz. Qayta belgilashni o'chirish yoki siyosatni qayta yoqish xabarni olib tashlaydi
SIMD-ni qo'llab-quvvatlaydigan hostlarda.

### Paritet tekshiruvlari

Deterministik natijalarni isbotlash uchun `AccelerationConfig` ni faqat protsessor va tezlashtirish o'rtasida aylantiring.
`poseidon_instructions_match_across_acceleration_configs` regressiyasi ishlaydi
Poseidon2/6 opkodlari ikki marta - avval `enable_cuda`/`enable_metal` bilan `false`, keyin
ikkalasi ham yoqilgan va GPU mavjud boʻlganda bir xil chiqishlar va CUDA paritetini tasdiqlaydi.【crates/ivm/tests/crypto.rs:100】
`acceleration_runtime_status()`-ni ishga tushirish bilan bir qatorda orqa uchlarini yozib oling
laboratoriya jurnallarida sozlangan/mavjud.

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### GPU standart sozlamalari va evristika

`MerkleTree` GPU yuklanishi sukut bo'yicha `8192` da boshlanadi va CPU SHA-2
afzallik chegaralari har bir arxitektura uchun `32_768` barglarida qoladi. Qachon na CUDA, na
Metall mavjud yoki sog'liq tekshiruvi natijasida o'chirilgan bo'lsa, VM avtomatik ravishda tushadi
SIMD/skalyar xeshga qaytish va yuqoridagi raqamlar determinizmga ta'sir qilmaydi.

`max_gpus`, `GpuManager` ga oziqlangan hovuz hajmini mahkamlaydi. `max_gpus = 1` sozlamalari yoqilgan
ko'p GPU xostlari telemetriyani sodda qilib, tezlashtirishga imkon beradi. Operatorlar mumkin
FASTPQ yoki CUDA Poseidon ishlari uchun qolgan qurilmalarni zaxiralash uchun ushbu kalitdan foydalaning.

### Keyingi tezlashtirish maqsadlari va byudjetlari

Oxirgi FastPQ metall izi (`fastpq_metal_bench_20k_latest.json`, 32K qator × 16
ustunlar, 5 iter) ZK ish yuklarida ustunlik qiluvchi Poseidon ustuni xeshini ko'rsatadi:

- `poseidon_hash_columns`: CPU o'rtacha **3,64s** va GPU o'rtacha **3,55s** (1,03×).
- `lde`: CPU o'rtacha **1,75s** va GPU o'rtacha **1,57s** (1,12×).

IVM/Crypto keyingi tezlashtirishda ushbu ikkita yadroni nishonga oladi. Asosiy byudjetlar:

- Skalar/SIMD paritetini yuqoridagi protsessor vositalarida yoki pastda saqlang va suratga oling
  Har bir yugurish bilan birga `acceleration_runtime_status()`, shuning uchun Metal/CUDA mavjudligi
  byudjet raqamlari bilan qayd etilgan.
- `poseidon_hash_columns` uchun maqsad ≥1,3× va `lde` uchun ≥1,2× bir marta sozlangan Metall
  va CUDA yadrolari chiqishlar yoki telemetriya belgilarini o'zgartirmasdan erga tushadi.

JSON trace va `cargo xtask acceleration-state --format json` oniy rasmini biriktiring
Kelajakdagi laboratoriya ishlaydi, shuning uchun CI/regressiyalar byudjetni ham, backend sog'lig'ini ham tasdiqlashi mumkin
faqat protsessor va tezlashtirishni taqqoslash.

### Norito evristikasi (kompilyatsiya vaqti standarti)Norito sxemasi va siqish evristikasi `crates/norito/src/core/heuristics.rs` da ishlaydi
va har bir binarga kompilyatsiya qilinadi. Ular ish vaqtida sozlanishi mumkin emas, lekin ochiq
Kirishlar SDK va operator guruhlariga Norito GPU bir marta oʻzini qanday tutishini bashorat qilishga yordam beradi
siqish yadrolari yoqilgan.
Ish maydoni endi sukut bo'yicha yoqilgan `gpu-compression` funksiyasi bilan Norito quradi,
shuning uchun GPU zstd backends kompilyatsiya qilinadi; ish vaqti mavjudligi hali ham apparatga bog'liq,
yordamchi kutubxona (`libgpuzstd_*`/`gpuzstd_cuda.dll`) va `allow_gpu_compression`
konfiguratsiya bayrog'i. `cargo build -p gpuzstd_metal --release` va yordamida Metall yordamchini yarating
`libgpuzstd_metal.dylib` ni yuklovchi yo'liga joylashtiring. Hozirgi metall yordamchisi GPU bilan ishlaydi
moslikni topish/ketma-ketlikni yaratish va kassa ichidagi deterministik zstd ramkasidan foydalanadi
xostdagi kodlovchi (Huffman/FSE + ramka yig'ish); dekodlash qutidagi ramkadan foydalanadi
GPU blokining dekodlanishi simli ulanmaguncha, qo'llab-quvvatlanmaydigan freymlar uchun CPU zstd zaxirasi bilan dekoder.

| Maydon | Standart | Maqsad |
|-------|---------|---------|
| `min_compress_bytes_cpu` | `256` bayt | Buning ostida, yuklamalar qo'shimcha xarajatlardan qochish uchun zstd to'liq o'tkazib yuboriladi. |
| `min_compress_bytes_gpu` | `1_048_576` bayt (1MiB) | `norito::core::hw::has_gpu_compression()` rost boʻlsa, bu chegaradagi yoki undan yuqori yuklar GPU zstd ga oʻtadi. |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | <32KiB va ≥32KiB foydali yuklar uchun mos ravishda CPU siqish darajalari. |
| `zstd_level_gpu` | `1` | Buyruqlar navbatlarini to'ldirishda kechikishni barqaror saqlash uchun konservativ GPU darajasi. |
| `large_threshold` | `32_768` bayt | "Kichik" va "katta" CPU zstd darajalari orasidagi o'lcham chegarasi. |
| `aos_ncb_small_n` | `64` qatorlar | Ushbu qatordan pastda moslashtirilgan enkoderlar eng kichik yukni tanlash uchun AoS va NCB sxemalarini tekshiradi. |
| `combo_no_delta_small_n_if_empty` | `2` qatorlar | 1–2 qatorda boʻsh katakchalar boʻlsa, u32/id delta kodlashlarini yoqishni oldini oladi. |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Deltalar kamida ikkita qator mavjud bo'lganda faqat bir marta tepiladi. |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | Barcha delta konvertatsiyalari yaxshi ishlaydigan kirishlar uchun sukut bo'yicha yoqilgan. |
| `combo_enable_name_dict` | `true` | Ustun koeffitsientlari xotira yukini oqlaganda har bir ustun lug'atlariga ruxsat beradi. |
| `combo_dict_ratio_max` | `0.40` | Qatorlarning 40% dan ortig'i farqlangan bo'lsa, lug'atlarni o'chirib qo'ying. |
| `combo_dict_avg_len_min` | `8.0` | Lug‘atlar yaratishdan oldin o‘rtacha satr uzunligi ≥8 bo‘lishini talab qiling (qisqa taxalluslar qatorda qoladi). |
| `combo_dict_max_entries` | `1024` | Cheklangan xotiradan foydalanishni kafolatlash uchun lug'at yozuvlari uchun qattiq qopqoq. |

Bu evristikalar GPU bilan ishlaydigan xostlarni faqat protsessorli tengdoshlar bilan moslashtiradi: selektor
hech qachon sim formatini o'zgartiradigan qaror qabul qilmaydi va chegaralar belgilanadi
chiqarish uchun. Profillash yaxshiroq zarar nuqtalarini ochganda, Norito yangilanadi
kanonik `Heuristics::canonical` ilovasi va `docs/source/benchmarks.md` plus
`status.md` versiyadagi dalillar bilan birga o'zgarishlarni yozib oling.GPU zstd yordamchisi hatto bir xil `min_compress_bytes_gpu` uzilishini amalga oshiradi.
to'g'ridan-to'g'ri chaqirilgan (masalan, `norito::core::gpu_zstd::encode_all` orqali), juda kichik
foydali yuklar GPU mavjudligidan qat'iy nazar har doim CPU yo'lida qoladi.

### Muammolarni bartaraf etish va paritetni tekshirish ro'yxati

- `cargo xtask acceleration-state --format json` bilan ish vaqti holatini suratga oling va saqlang
  har qanday muvaffaqiyatsiz jurnallar bilan birga chiqish; hisobot konfiguratsiya qilingan/mavjud backendlarni ko'rsatadi
  plyus parite/oxirgi xato satrlari.
- Driftni istisno qilish uchun tezlashuv pariteti regressiyasini lokal ravishda qayta ishga tushiring:
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (faqat protsessorda ishlaydi, keyin tezlashadi). Yugurish uchun `acceleration_runtime_status()` ni yozib oling.
- Agar server o'z-o'zini sinovdan o'tkazmasa, tugunni faqat CPU rejimida onlayn saqlang (`enable_metal =
  false`, `enable_cuda = false`) va olingan paritet chiqishi bilan hodisani oching
  orqa tomonni majburlash o'rniga. Natijalar rejimlarda deterministik bo'lib qolishi kerak.
- **CUDA pariteti tutuni (laboratoriya NV apparati):** Run
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x uskunasida `cargo xtask acceleration-state --format json` ni oling va biriktiring
  mezon artefaktlariga holat surati (GPU modeli/drayveri kiritilgan).