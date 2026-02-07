---
lang: mn
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Хурдасгах & Norito Хевристикийн лавлагаа

`iroha_config` дахь `[accel]` блок нь дамжуулан дамжуулдаг.
`crates/irohad/src/main.rs:1895` `ivm::set_acceleration_config` руу. Хөтлөгч бүр
нь VM-г үүсгэхээс өмнө ижил товчлууруудыг ашигладаг тул операторууд тодорхойлох боломжтой
скаляр/SIMD нөөцийг ашиглах боломжтой байлгахын зэрэгцээ аль GPU арын хэсэгт зөвшөөрөгдөхийг сонгоно уу.
Swift, Android болон Python холболтууд нь ижил манифестийг гүүрний давхаргаар ачаалдаг
Эдгээр өгөгдмөлүүдийг баримтжуулах нь техник хангамжийн хурдатгалын хоцрогдол дахь WP6-C-ийн блокыг арилгана.

### `accel` (техник хангамжийн хурдатгал)

Доорх хүснэгтэд `docs/source/references/peer.template.toml` болон
`iroha_config::parameters::user::Acceleration` тодорхойлолт, хүрээлэн буй орчныг ил гаргах
түлхүүр бүрийг дарах хувьсагч.

| Түлхүүр | Env var | Өгөгдмөл | Тодорхойлолт |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX гүйцэтгэлийг идэвхжүүлдэг. `false` үед VM нь детерминист паритетийг барихад хялбар болгохын тулд вектор үйлдлийн болон Меркле хэшэнд зориулсан скаляр арын хэсгийг албаддаг. |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | CUDA backend-ийг хөрвүүлж, ажиллах хугацаа нь бүх алтан вектор шалгалтыг давсан үед идэвхжүүлнэ. |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | MacOS бүтээцүүд дээр Металл арын хэсгийг идэвхжүүлдэг. Үнэн байсан ч, хэрэв паритын зөрүү гарсан тохиолдолд метал өөрөө тест нь ажиллах үед арын хэсгийг идэвхгүй болгож чадна. |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (авто) | Ажиллах цаг нь хэдэн физик GPU-г эхлүүлж байгааг тэмдэглэнэ. `0` нь "тохирох техник хангамж" гэсэн утгатай бөгөөд `GpuManager`-ээр хавчуулсан. |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Merkle навчийг GPU руу хэшлэхээс өмнө хамгийн бага навч шаардлагатай. Энэ босго хэмжээнээс доогуур утгууд нь PCIe-ийн хэт ачааллаас зайлсхийхийн тулд CPU дээр үргэлжлүүлэн хэш (`crates/ivm/src/byte_merkle_tree.rs:49`). |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (дэлхийн удамшлын) | GPU-ийн босгыг металлын тусгайлан дарах. `0` үед Метал нь `merkle_min_leaves_gpu`-ийг өвлөнө. |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (дэлхийн өвлөн авах) | GPU босгыг CUDA-д зориулсан тусгай хүчингүй болгох. `0` үед CUDA нь `merkle_min_leaves_gpu`-г өвлөнө. |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (дотооддоо 32768) | ARMv8 SHA-2 заавар нь GPU хэшийг давах ёстой модны хэмжээг хязгаарлана. `0` нь `32_768` (`crates/ivm/src/byte_merkle_tree.rs:59`) хуудасны эмхэтгэсэн өгөгдмөлийг хадгалдаг. |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (дотооддоо 32768) | Дээрхтэй ижил боловч SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) ашигладаг x86/x86_64 хостуудад зориулагдсан. |

`enable_simd` нь мөн RS16 устгах кодчиллыг (Torii DA залгих + багаж хэрэгсэл) хянадаг. Үүнийг идэвхгүй болгох
скаляр паритет үүсгэхийг хүчлэхийн зэрэгцээ гаралтыг техник хангамжийн хэмжээнд тодорхойлогч байлгах.

Жишээ тохиргоо:

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

Сүүлийн таван товчлуурын тэг утгууд нь "эмхэтгэсэн өгөгдмөлийг хадгалах" гэсэн үг юм. Хостууд тэгэх ёсгүй
зөрчилтэй хүчингүй болгох (жишээ нь, зөвхөн CUDA-н босгыг албадах үед CUDA-г идэвхгүй болгох),
эс бөгөөс хүсэлтийг үл тоомсорлож, арын хэсэг нь дэлхийн бодлогыг үргэлжлүүлэн дагаж мөрддөг.

### Ажиллах цагийн төлөвийг шалгаж байнаХэрэглээний зургийг авахын тулд `cargo xtask acceleration-state [--format table|json]`-г ажиллуулна уу
Метал/CUDA ажиллах үеийн эрүүл мэндийн битүүдийн зэрэгцээ тохиргоо. Команд нь татдаг
одоогийн `ivm::acceleration_config`, тэгш байдлын төлөв, наалттай алдааны мөрүүд (хэрэв
backend идэвхгүй болсон) тул үйлдлүүд нь үр дүнг шууд паритет руу оруулах боломжтой
хяналтын самбар эсвэл ослын тойм.

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

Зургийг автоматжуулалтаар оруулах шаардлагатай үед `--format json` ашиглана уу (JSON).
хүснэгтэд үзүүлсэн ижил талбаруудыг агуулна).

`acceleration_runtime_errors()` одоо SIMD яагаад скаляр болж хувирсныг дуудаж байна:
`disabled by config`, `forced scalar override`, `simd unsupported on hardware`, эсвэл
Илрүүлэх амжилттай үед `simd unavailable at runtime` боловч гүйцэтгэл ажиллаж байна
векторгүй. Хүчингүй болгохыг арилгах эсвэл бодлогыг дахин идэвхжүүлснээр мессеж алга болно
SIMD-г дэмждэг хостууд дээр.

### Паритет шалгах

Тодорхой үр дүнг батлахын тулд `AccelerationConfig`-г зөвхөн CPU болон хурдатгалын хооронд эргүүлээрэй.
`poseidon_instructions_match_across_acceleration_configs` регресс нь
Poseidon2/6 үйлдлийн кодыг хоёр удаа хийнэ—эхлээд `enable_cuda`/`enable_metal`-г `false` болгож, дараа нь
хоёуланг нь идэвхжүүлсэн - мөн GPU байгаа үед ижил гаралт болон CUDA паритетыг баталгаажуулдаг.【crates/ivm/tests/crypto.rs:100】
`acceleration_runtime_status()`-г гүйлтийн хажуугаар зурж, арын төгсгөл байгаа эсэхийг бичнэ үү
лабораторийн бүртгэлд тохируулагдсан/боломжтой байсан.

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

### GPU өгөгдмөл & эвристик

`MerkleTree` GPU ачааллыг `8192` үед эхлүүлэх ба CPU SHA-2
Архитектур бүрт давуу эрх олгох босго `32_768` хэвээр байна. Хэзээ ч CUDA ч биш
Металл байгаа эсвэл эрүүл мэндийн үзлэгээр идэвхгүй болсон тохиолдолд VM автоматаар унана
SIMD/скаляр хэш рүү буцах ба дээрх тоонууд детерминизмд нөлөөлөхгүй.

`max_gpus` `GpuManager`-д тэжээгддэг усан сангийн хэмжээг хавчаарлана. `max_gpus = 1` тохиргоог асааж байна
олон GPU хостууд нь хурдатгал хийх боломжийг олгохын зэрэгцээ телеметрийг энгийн байлгадаг. Операторууд чадна
FASTPQ эсвэл CUDA Poseidon ажилд зориулж үлдсэн төхөөрөмжүүдийг нөөцлөхийн тулд энэ шилжүүлэгчийг ашиглана уу.

### Дараагийн хурдатгалын зорилтууд ба төсөв

Хамгийн сүүлийн үеийн FastPQ Металл мөр (`fastpq_metal_bench_20k_latest.json`, 32K мөр × 16
багана, 5 давталт) нь ZK ажлын ачаалал давамгайлж буй Посейдон баганын хэшийг харуулж байна:

- `poseidon_hash_columns`: CPU-ийн дундаж **3.64с** ба GPU-ийн дундаж **3.55с** (1.03×).
- `lde`: CPU-ийн дундаж **1.75с** болон GPU-ийн дундаж **1.57s** (1.12×).

IVM/Crypto нь дараагийн хурдатгалын үед эдгээр хоёр цөмийг онилно. Суурь төсөв:

- Скаляр/СИМД-ийн паритетийг CPU-ийн утсан дээр эсвэл доор байлгаж, барьж ав
  `acceleration_runtime_status()` гүйлт бүрийн хажуугаар Металл/CUDA ашиглах боломжтой
  төсвийн тоогоор бүртгүүлсэн.
- Нэг удаа тааруулсан Металл `poseidon_hash_columns`-ийн зорилтот ≥1.3× хурд, `lde`-ийн хувьд ≥1.2×
  болон CUDA цөмүүд гаралт болон телеметрийн шошгыг өөрчлөхгүйгээр газарддаг.

JSON мөр болон `cargo xtask acceleration-state --format json` агшин зуурын зургийг хавсаргана уу
ирээдүйн лаборатори ажилладаг тул CI/регрессүүд нь төсөв болон арын хэсгийн эрүүл мэндийг хоёуланг нь баталж чадна
Зөвхөн CPU болон хурдатгалтай гүйлтүүдийг харьцуулах.

### Norito эвристик (эмхэтгэх хугацааны өгөгдмөл)Norito-ийн зохион байгуулалт, шахалтын эвристик нь `crates/norito/src/core/heuristics.rs` дээр амьдардаг
бөгөөд хоёртын файл болгонд хөрвүүлэгддэг. Тэдгээрийг ажиллах үед тохируулах боломжгүй, харин ил гаргах боломжтой
оролтууд нь SDK болон операторын багуудад GPU нэг удаа Norito хэрхэн ажиллахыг таамаглахад тусалдаг.
шахалтын цөмүүдийг идэвхжүүлсэн.
Ажлын талбар нь одоо Norito-г анхдагчаар идэвхжүүлсэн `gpu-compression` функцээр бүтээж байна.
Тиймээс GPU zstd арын хэсгүүдийг эмхэтгэсэн; ажиллах цагийн хүртээмж нь техник хангамжаас хамааралтай хэвээр байна.
туслах номын сан (`libgpuzstd_*`/`gpuzstd_cuda.dll`), `allow_gpu_compression`
тохиргооны туг. Металл туслагчийг `cargo build -p gpuzstd_metal --release` болон
`libgpuzstd_metal.dylib`-ийг ачигчийн зам дээр байрлуулна. Одоогийн Метал туслагч нь GPU-г ажиллуулдаг
таарч олох/дараалал үүсгэх ба крат доторх детерминист zstd фреймийг ашигладаг
хост дээрх кодлогч (Huffman/FSE + хүрээ угсралт); декодчилол нь хайрцаг доторх хүрээг ашигладаг
GPU блокийн кодыг тайлах утастай болтол дэмжигдээгүй фреймд зориулсан CPU zstd нөөцтэй декодчилогч.

| Талбай | Өгөгдмөл | Зорилго |
|-------|---------|---------|
| `min_compress_bytes_cpu` | `256` байт | Үүний доор ачааллын ачааллаас зайлсхийхийн тулд zstd-г бүхэлд нь алгасдаг. |
| `min_compress_bytes_gpu` | `1_048_576` байт (1МиБ) | `norito::core::hw::has_gpu_compression()` үнэн үед энэ хязгаараас дээш хэмжээтэй ачаалал GPU zstd руу шилжинэ. |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | <32KiB ба ≥32KiB ачааллын хувьд CPU шахалтын түвшин. |
| `zstd_level_gpu` | `1` | Командын дарааллыг бөглөх явцад хоцролтыг тогтвортой байлгахын тулд GPU-ийн консерватив түвшин. |
| `large_threshold` | `32_768` байт | "Жижиг" ба "том" CPU zstd түвшний хоорондох хэмжээсийн хил хязгаар. |
| `aos_ncb_small_n` | `64` мөр | Энэ эгнээний доор дасан зохицох кодлогч нь хамгийн бага ачааллыг сонгохын тулд AoS болон NCB байршлыг хоёуланг нь шалгадаг. |
| `combo_no_delta_small_n_if_empty` | `2` мөр | 1–2 мөрөнд хоосон нүд байх үед u32/id дельта кодчиллыг идэвхжүүлэхээс сэргийлнэ. |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Дельта нь дор хаяж хоёр эгнээ байх үед л нэг удаа өшиглөнө. |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | Бүх дельта хувиргалтыг сайн ажиллагаатай оролтын хувьд анхдагчаар идэвхжүүлдэг. |
| `combo_enable_name_dict` | `true` | Даралтын харьцаа нь санах ойн ачааллыг зөвтгөх үед багана тус бүрийн толь бичгийг зөвшөөрдөг. |
| `combo_dict_ratio_max` | `0.40` | Мөрүүдийн 40% -иас илүү нь ялгаатай үед толь бичгийг идэвхгүй болго. |
| `combo_dict_avg_len_min` | `8.0` | Толь бичиг бүтээхээс өмнө мөрийн дундаж урт ≥8 байх шаардлагатай (богино нэр нь шугаманд үлдэнэ). |
| `combo_dict_max_entries` | `1024` | Хязгаарлагдмал санах ойн ашиглалтыг баталгаажуулахын тулд толь бичгийн оруулгуудыг хатуу таглана. |

Эдгээр эвристик нь GPU-г идэвхжүүлсэн хостуудыг зөвхөн CPU-тэй ижил түвшинд байлгадаг: сонгогч
Утасны форматыг өөрчлөх шийдвэр хэзээ ч гаргадаггүй бөгөөд босго нь тогтмол байдаг
хувилбар бүрт. Профайл нь илүү сайн эвдэх цэгүүдийг илрүүлэх үед Norito
каноник `Heuristics::canonical` хэрэгжилт ба `docs/source/benchmarks.md` plus
`status.md` өөрчлөлтийг хувилбартай нотлох баримтын хамт тэмдэглэнэ.GPU zstd туслагч нь ижил `min_compress_bytes_gpu` таслалтыг хэрэгжүүлдэг.
шууд дуудагдсан (жишээ нь `norito::core::gpu_zstd::encode_all`-ээр), маш жижиг
Ачаалал нь GPU-ийн боломжоос үл хамааран CPU-ийн зам дээр үргэлж үлддэг.

### Алдааг олж засварлах болон тэгш байдлыг шалгах хуудас

- `cargo xtask acceleration-state --format json`-ийн агшин зуурын агшин зуурын төлөвийг хадгалах
  ямар ч бүтэлгүйтсэн логуудын зэрэгцээ гаралт; тайлан нь тохируулсан/боломжтой арын хэсгүүдийг харуулж байна
  нэмэх парити/сүүлийн алдааны мөрүүд.
- Дрифтийг үгүйсгэхийн тулд хурдатгалын паритын регрессийг дотооддоо дахин ажиллуулна уу:
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (зөвхөн CPU-г ажиллуулж, дараа нь хурдасгах). Гүйлтийн хувьд `acceleration_runtime_status()` гэж бичээрэй.
- Хэрэв арын хэсэг өөрөө өөрийгөө шалгахад амжилтгүй болбол зангилааг зөвхөн CPU горимд онлайн байлга (`enable_metal =
  false`, `enable_cuda = false`) болон баригдсан паритын гаралттай тохиолдлыг нээх.
  арын хэсгийг хүчээр асаахын оронд. Үр дүн нь янз бүрийн горимд тодорхойлогч хэвээр байх ёстой.
- **CUDA паритын утаа (лабораторийн NV техник хангамж):** Ажиллуулах
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x тоног төхөөрөмж дээр `cargo xtask acceleration-state --format json`-г барьж аваад хавсаргана уу
  жишиг олдворуудын статусын агшин зуурын зураг (GPU загвар/драйвер багтсан).