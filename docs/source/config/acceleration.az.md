---
lang: az
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Sürətləndirmə və Norito Evristik İstinad

`iroha_config`-dəki `[accel]` bloku vasitəsilə
`crates/irohad/src/main.rs:1895` - `ivm::set_acceleration_config`. Hər ev sahibi
VM-i işə salmazdan əvvəl eyni düymələri tətbiq edir, beləliklə operatorlar müəyyən edə bilsinlər
skalyar/SIMD ehtiyatlarını əlçatan saxlayarkən hansı GPU arxa tərəflərinə icazə verildiyini seçin.
Swift, Android və Python bağlamaları eyni manifesti körpü qatı vasitəsilə yükləyir
bu defoltların sənədləşdirilməsi WP6-C-ni aparat sürətləndirilməsi üzrə blokdan çıxarır.

### `accel` (avadanlığın sürətləndirilməsi)

Aşağıdakı cədvəldə `docs/source/references/peer.template.toml` və
Ətraf mühiti ifşa edən `iroha_config::parameters::user::Acceleration` tərifi
hər bir açarı əvəz edən dəyişən.

| Açar | Env var | Defolt | Təsvir |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX icrasını aktivləşdirir. `false` zaman, VM vektor əməliyyatları və deterministik paritet tutmalarını asanlaşdırmaq üçün Merkle heshing üçün skalyar arxa uçları məcbur edir. |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | CUDA backend tərtib edildikdə və iş vaxtı bütün qızıl vektor yoxlamalarından keçdikdə onu işə salır. |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | macOS konstruksiyalarında Metal arxa ucunu aktivləşdirir. Hətta doğru olsa belə, paritet uyğunsuzluqları baş verərsə, Metal özünü testlər hələ də işləmə zamanı backendi söndürə bilər. |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (avtomatik) | İş vaxtının nə qədər fiziki GPU-nun işə salındığını qeyd edir. `0` "uyğun aparat fan-out" deməkdir və `GpuManager` tərəfindən sıxılır. |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Merkle yarpağı heşinqi GPU-ya yüklənmədən əvvəl tələb olunan minimum yarpaqlar. Bu həddən aşağı olan dəyərlər PCIe yükünün qarşısını almaq üçün CPU-da hashing etməyə davam edir (`crates/ivm/src/byte_merkle_tree.rs:49`). |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (qlobal varis) | GPU həddi üçün metal-xüsusi əvəzetmə. `0` olduqda, Metal `merkle_min_leaves_gpu`-i miras alır. |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (qlobal varis) | GPU həddi üçün CUDA-ya xas üstəlik. `0` zaman, CUDA `merkle_min_leaves_gpu` miras alır. |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (daxili olaraq 32768) | ARMv8 SHA-2 təlimatlarının GPU hashing üzərində qalib gəldiyi ağac ölçüsünü əhatə edir. `0` `32_768` yarpaqlarının tərtib edilmiş defoltunu saxlayır (`crates/ivm/src/byte_merkle_tree.rs:59`). |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (daxili olaraq 32768) | Yuxarıdakı kimi, lakin SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) istifadə edən x86/x86_64 hostları üçün. |

`enable_simd` həmçinin RS16 silmə kodlamasına nəzarət edir (Torii DA qəbulu + alətlər). Onu söndürün
hardware arasında deterministik çıxışları saxlayaraq skalyar paritet yaratmağa məcbur edin.

Misal konfiqurasiya:

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

Son beş düymə üçün sıfır dəyərlər "tərtib edilmiş standartı saxlamaq" deməkdir. Ev sahibləri olmamalıdır
ziddiyyətli ləğvetmələri təyin edin (məsələn, yalnız CUDA hədlərini məcbur edərkən CUDA-nı söndürmək),
əks halda sorğuya məhəl qoyulmur və backend qlobal siyasətə əməl etməyə davam edir.

### İş vaxtı vəziyyəti yoxlanılırTətbiq olunanın şəklini çəkmək üçün `cargo xtask acceleration-state [--format table|json]`-i işə salın
Metal/CUDA iş vaxtı sağlamlıq bitləri ilə yanaşı konfiqurasiya. Komanda çəkir
cari `ivm::acceleration_config`, paritet statusu və yapışqan xəta sətirləri (əgər
backend qeyri-aktiv edildi) beləliklə əməliyyatlar nəticəni birbaşa paritetə çatdıra bilər
tablosuna və ya hadisə icmalı.

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

Snapshotın avtomatlaşdırma (JSON) tərəfindən qəbul edilməsi lazım olduqda `--format json` istifadə edin
cədvəldə göstərilən eyni sahələri ehtiva edir).

`acceleration_runtime_errors()` indi SIMD-nin niyə yenidən skalaya düşdüyünü soruşur:
`disabled by config`, `forced scalar override`, `simd unsupported on hardware` və ya
`simd unavailable at runtime` aşkarlama uğurlu olduqda, lakin icra hələ də davam edir
vektorlar olmadan. Ləğvetmənin silinməsi və ya siyasətin yenidən aktivləşdirilməsi mesajı buraxır
SIMD-ni dəstəkləyən hostlarda.

### Paritetin yoxlanılması

Deterministik nəticələri sübut etmək üçün `AccelerationConfig`-i yalnız CPU və sürətləndirmə arasında çevirin.
`poseidon_instructions_match_across_acceleration_configs` reqressiyası işləyir
Poseidon2/6 əməliyyat kodları iki dəfə - əvvəlcə `enable_cuda`/`enable_metal` ilə `false`, sonra
hər ikisi aktivdir və GPU-lar mövcud olduqda eyni çıxışları və CUDA paritetini təsdiqləyir.【crates/ivm/tests/crypto.rs:100】
Arxa uçların olub olmadığını qeyd etmək üçün qaçışla yanaşı `acceleration_runtime_status()`-i çəkin
konfiqurasiya edilmiş/laborator jurnallarında mövcud olmuşdur.

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

### GPU defoltları və evristika

`MerkleTree` GPU yüklənməsi `8192`-də başlayır və defolt olaraq CPU SHA-2 çıxır.
üstünlük hədləri memarlıq üçün `32_768` yarpaqlarında qalır. Nə CUDA, nə də
Metal mövcuddur və ya sağlamlıq yoxlamaları ilə aradan qaldırılıb, VM avtomatik olaraq düşür
SIMD/scalar hashing-ə qayıdın və yuxarıdakı rəqəmlər determinizmə təsir göstərmir.

`max_gpus`, `GpuManager`-ə qidalanan hovuz ölçüsünü sıxır. `max_gpus = 1` parametri aktivdir
multi-GPU hostları sürətləndirməyə imkan verərkən telemetriyanı sadə saxlayır. Operatorlar bilər
FASTPQ və ya CUDA Poseidon işləri üçün qalan cihazları rezerv etmək üçün bu keçiddən istifadə edin.

### Növbəti sürətləndirmə hədəfləri və büdcələri

Ən son FastPQ Metal izi (`fastpq_metal_bench_20k_latest.json`, 32K sıra × 16
sütunlar, 5 iter) ZK iş yüklərində üstünlük təşkil edən Poseidon sütununun heshingini göstərir:

- `poseidon_hash_columns`: CPU orta **3,64s** və GPU ortalaması **3,55s** (1,03×).
- `lde`: CPU orta **1,75s** və GPU orta **1,57s** (1,12×).

IVM/Crypto növbəti sürətləndirmədə bu iki nüvəni hədəf alacaq. İlkin büdcələr:

- Skalar/SIMD paritetini yuxarıdakı CPU vasitələrində və ya aşağıda saxlayın və ələ keçirin
  `acceleration_runtime_status()` hər qaçışla yanaşı, Metal/CUDA-nın mövcudluğu belədir
  büdcə nömrələri ilə daxil edilir.
- `poseidon_hash_columns` üçün hədəf ≥1.3× sürətləndirmə və `lde` üçün ≥1.2× bir dəfə köklənmiş Metal
  və CUDA ləpələri çıxışları və ya telemetriya etiketlərini dəyişdirmədən yerə enir.

JSON izi və `cargo xtask acceleration-state --format json` snapşotunu əlavə edin
gələcək laboratoriya belə işləyir ki, CI/reqressiyalar həm büdcələri, həm də arxa uçun sağlamlığını təsdiq edə bilsin
yalnız CPU ilə sürətləndirici işlərin müqayisəsi.

### Norito evristikası (kompilyasiya vaxtı defoltları)Norito-in tərtibatı və sıxılma evristikası `crates/norito/src/core/heuristics.rs`-də yaşayır
və hər ikiliyə tərtib edilir. Onlar iş vaxtında konfiqurasiya edilə bilməz, lakin ifşa olunur
girişlər SDK və operator komandalarına GPU-dan sonra Norito-in necə davranacağını proqnozlaşdırmağa kömək edir
sıxılma nüvələri aktivləşdirilib.
İş sahəsi indi defolt olaraq aktivləşdirilmiş `gpu-compression` funksiyası ilə Norito qurur,
beləliklə, GPU zstd arxa uçları tərtib edilir; iş vaxtının mövcudluğu hələ də aparatdan asılıdır,
köməkçi kitabxana (`libgpuzstd_*`/`gpuzstd_cuda.dll`) və `allow_gpu_compression`
konfiqurasiya bayrağı. `cargo build -p gpuzstd_metal --release` və ilə Metal köməkçisini yaradın
yükləyici yoluna `libgpuzstd_metal.dylib` qoyun. Hazırkı Metal köməkçisi GPU ilə işləyir
uyğunluq tapmaq/ardıcıllıq yaratmaq və qutuda deterministik zstd çərçivəsini istifadə edir
hostda kodlayıcı (Huffman/FSE + çərçivə montajı); deşifrə qutudaxili çərçivədən istifadə edir
GPU blokunun deşifrəsi kabelə qoşulana qədər dəstəklənməyən çərçivələr üçün CPU zstd ehtiyatı ilə dekoder.

| Sahə | Defolt | Məqsəd |
|-------|---------|---------|
| `min_compress_bytes_cpu` | `256` bayt | Bunun altında yük yüklərinin qarşısını almaq üçün zstd tamamilə atlanır. |
| `min_compress_bytes_gpu` | `1_048_576` bayt (1MiB) | `norito::core::hw::has_gpu_compression()` doğru olduqda bu limitdə və ya yuxarıda olan faydalı yüklər GPU zstd-ə keçir. |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | <32KiB və ≥32KiB faydalı yüklər üçün CPU sıxılma səviyyələri. |
| `zstd_level_gpu` | `1` | Komanda növbələrini doldurarkən gecikməni ardıcıl saxlamaq üçün mühafizəkar GPU səviyyəsi. |
| `large_threshold` | `32_768` bayt | “Kiçik” və “böyük” CPU zstd səviyyələri arasında ölçü sərhədi. |
| `aos_ncb_small_n` | `64` sətirlər | Bu sıranın altında uyğunlaşmalı kodlayıcılar ən kiçik yükü seçmək üçün həm AoS, həm də NCB planlarını yoxlayır. |
| `combo_no_delta_small_n_if_empty` | `2` sətirlər | 1-2 cərgədə boş xanalar olduqda u32/id delta kodlaşdırmalarının işə salınmasının qarşısını alır. |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Deltalar ən azı iki sıra olduqda yalnız bir dəfə təpiklənir. |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | Bütün delta çevrilmələri düzgün idarə olunan girişlər üçün standart olaraq aktivləşdirilir. |
| `combo_enable_name_dict` | `true` | Hücum nisbətləri yaddaş yükünü əsaslandırdıqda sütun başına lüğətlərə icazə verir. |
| `combo_dict_ratio_max` | `0.40` | Sətirlərin 40%-dən çoxu fərqli olduqda lüğətləri deaktiv edin. |
| `combo_dict_avg_len_min` | `8.0` | Lüğətlər yaratmazdan əvvəl orta sətir uzunluğunun ≥8 olmasını tələb edin (qısa ləqəblər sətirdə qalır). |
| `combo_dict_max_entries` | `1024` | Məhdud yaddaş istifadəsinə zəmanət vermək üçün lüğət girişlərində sərt qapaq. |

Bu evristikalar GPU-nu aktivləşdirən hostları yalnız CPU-nun həmyaşıdları ilə uyğunlaşdırır: seçici
heç vaxt tel formatını dəyişdirəcək qərar qəbul etmir və həddlər müəyyən edilir
buraxılış başına. Profil yaratmaq daha yaxşı zərərsizlik nöqtələrini aşkar edərkən, Norito
kanonik `Heuristics::canonical` tətbiqi və `docs/source/benchmarks.md` plus
`status.md` dəyişikliyi versiyalı sübutlarla birlikdə qeyd edin.GPU zstd köməkçisi hətta eyni `min_compress_bytes_gpu` kəsilməsini tətbiq edir.
birbaşa çağırılır (məsələn, `norito::core::gpu_zstd::encode_all` vasitəsilə), belə kiçik
faydalı yüklər GPU-nun mövcudluğundan asılı olmayaraq həmişə CPU yolunda qalır.

### Problemlərin aradan qaldırılması və paritet yoxlama siyahısı

- `cargo xtask acceleration-state --format json` ilə snapshot iş vaxtı vəziyyəti və saxlayın
  hər hansı bir uğursuz jurnalla yanaşı çıxış; hesabat konfiqurasiya edilmiş/mövcud backendləri göstərir
  üstəgəl paritet/son xəta sətirləri.
- Sürtünməni istisna etmək üçün sürət pariteti reqressiyasını lokal olaraq yenidən işlədin:
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (yalnız CPU-da işləyir, sonra sürətlənir). Qaçış üçün `acceleration_runtime_status()` yazın.
- Əgər backend özünü sınamaqda uğursuz olarsa, nodu yalnız CPU rejimində onlayn saxlayın (`enable_metal =
  false`, `enable_cuda = false`) və tutulan paritet çıxışı ilə insident açın
  arxa ucunu məcbur etmək əvəzinə. Nəticələr rejimlər arasında deterministik qalmalıdır.
- **CUDA paritet tüstüsü (laboratoriya NV aparatı):** Çalışın
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x aparatında, `cargo xtask acceleration-state --format json`-i çəkin və əlavə edin
  etalon artefaktlara status snapshot (GPU modeli/sürücü daxildir).