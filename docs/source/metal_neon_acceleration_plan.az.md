---
lang: az
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2025-12-29T18:16:35.976997+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Metal və NEON Sürətləndirmə Planı (Swift & Rust)

Bu sənəd deterministik avadanlığı işə salmaq üçün paylaşılan planı əks etdirir
sürətləndirmə (Metal GPU + NEON/Accelerate SIMD + StrongBox inteqrasiyası)
Rust iş sahəsi və Swift SDK. O, izlənilən yol xəritəsi elementlərinə müraciət edir
**Hardware Acceleration Workstream (macOS/iOS)** altında və ötürülmə təmin edir
Rust IVM komandası, Swift körpü sahibləri və telemetriya alətləri üçün artefakt.

> Son yeniləmə: 2026-01-12  
> Sahiblər: IVM Performans TL, Swift SDK Rəhbəri

## Məqsədlər

1. Metal vasitəsilə Apple aparatında Rust GPU ləpələrini (Poseidon/BN254/CRC64) təkrar istifadə edin
   CPU yollarına qarşı deterministik pariteti olan hesablama şeyderləri.
2. Sürətləndirmə keçidlərini (`AccelerationConfig`) ucdan-uca çıxarın ki, Swift tətbiqləri olsun
   ABI/paritet zəmanətlərini qoruyarkən Metal/NEON/StrongBox-a daxil ola bilər.
3. Parite/benchmark məlumatlarını və bayraqları üzə çıxarmaq üçün CI + idarə panellərini quraşdırın
   CPU və GPU/SIMD yolları arasında reqressiyalar.
4. Android (AND2) və Swift arasında StrongBox/təhlükəsiz anklav dərslərini paylaşın
   (IOS4) imzalama axınlarını deterministik şəkildə uyğunlaşdırmaq üçün.

**Yeniləmə (CRC64 + Mərhələ-1 yeniləmə):** CRC64 GPU köməkçiləri indi 192KiB defolt kəsmə ilə `norito::core::hardware_crc64`-ə qoşulub (`NORITO_GPU_CRC64_MIN_BYTES` və ya açıq-aşkar yardımçı yolu Prometheus vasitəsilə ləğv edilir) və SIM3 geriləmə zamanı. JSON Mərhələ-1 kəsmələri yenidən müqayisə edildi (`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`), skalyar kəsməni 4KiB-də saxladı və Mərhələ-1 GPU defoltunu 192KiB-ə uyğunlaşdırdı (`NORITO_STAGE1_GPU_MIN_BYTES` və GPU-da işləmək üçün böyük xərclər və kiçik ödənişli sənədlər)

## Çatdırılanlar və Sahiblər

| Mərhələ | Çatdırılır | Sahib(lər) | Hədəf |
|----------|-------------|----------|--------|
| Rust WP2-A/B | CUDA nüvələrini əks etdirən metal şader interfeysləri | IVM Perf TL | Fevral 2026 |
| Rust WP2-C | Metal BN254 paritet testləri və CI zolağı | IVM Perf TL | Q2 2026 |
| Swift IOS6 | Körpü keçidləri simli (`connect_norito_set_acceleration_config`) + SDK API + nümunələri | Swift Bridge Sahibləri | Tamamlandı (Yanvar 2026) |
| Swift IOS5 | Konfiqurasiya istifadəsini nümayiş etdirən nümunə proqramlar/sənədlər | Swift DX TL | Q2 2026 |
| Telemetriya | Dashboard, sürətləndirmə pariteti + etalon ölçüləri ilə qidalanır | Swift Proqramı PM / Telemetriya | Pilot məlumatları 2026-cı ilin 2-ci rübü |
| CI | XCFramework tüstü kəməri cihaz hovuzunda CPU vs Metal/NEON | Swift QA Rəhbəri | Q2 2026 |
| StrongBox | Aparat tərəfindən dəstəklənən imza pariteti testləri (paylaşılan vektorlar) | Android Crypto TL / Swift Təhlükəsizlik | 3-cü rüb 2026 |

## İnterfeyslər və API Müqavilələri### Pas (`ivm::AccelerationConfig`)
- Mövcud sahələri saxlayın (`enable_simd`, `enable_metal`, `enable_cuda`, `max_gpus`, eşiklər).
- İlk istifadə gecikməsinin qarşısını almaq üçün açıq Metal istiləşmə əlavə edin (Rust #15875).
- Panellər üçün status/diaqnostika qaytaran paritet API təmin edin:
  - məs. `ivm::vector::metal_status()` -> {aktiv, paritet, sonuncu_xəta}.
- Çıxış müqayisəsi ölçüləri (Merkle ağac vaxtları, CRC məhsuldarlığı) vasitəsilə
  `ci/xcode-swift-parity` üçün telemetriya qarmaqları.
- Metal host indi tərtib edilmiş `fastpq.metallib` yükləyir, FFT/IFFT/LDE göndərir
  və Poseidon ləpələri və hər dəfə CPU tətbiqinə qayıdır
  metallib və ya cihaz növbəsi mövcud deyil.

### C FFI (`connect_norito_bridge`)
- Yeni struktur `connect_norito_acceleration_config` (tamamlandı).
- Getter əhatə dairəsinə indi təyinedicini əks etdirmək üçün `connect_norito_get_acceleration_config` (yalnız konfiqurasiya) və `connect_norito_get_acceleration_state` (konfiqurasiya + paritet) daxildir.
- SPM/CocoaPods istehlakçıları üçün başlıq şərhlərində sənəd strukturunun tərtibatı.

### Swift (`AccelerationSettings`)
- Defoltlar: Metal aktivdir, CUDA deaktivdir, eşiklər sıfırdır (miras).
- Mənfi dəyərlər nəzərə alınmır; `apply()`, `IrohaSDK` tərəfindən avtomatik olaraq çağırılır.
- `AccelerationSettings.runtimeState()` indi `connect_norito_get_acceleration_state` səthini göstərir
  faydalı yük (konfiqurasiya + Metal/CUDA pariteti statusu) belə ki, Swift panelləri eyni telemetriyanı yayır
  Rust kimi (`supported/configured/available/parity`). Köməkçi olduqda `nil` qaytarır
  Testləri portativ saxlamaq üçün körpü yoxdur.
- `AccelerationBackendStatus.lastError`-dən söndürmə/səhv səbəbini kopyalayır
  `connect_norito_get_acceleration_state` və sətir olduqdan sonra yerli buferi azad edir
  Mobil paritet panelləri Metal/CUDA-nın niyə söndürüldüyünü izah edə bilsin
  hər bir ev sahibi.
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  indi `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift` altında testlər
  operator manifestlərini Norito demosu ilə eyni prioritet qaydada həll edir: honor
  `NORITO_ACCEL_CONFIG_PATH`, axtarış paketi `acceleration.{json,toml}` / `client.{json,toml}`,
  seçilmiş mənbəni daxil edin və defoltlara qayıdın. Tətbiqlərin daha sifarişli yükləyicilərə ehtiyacı yoxdur
  Rust `iroha_config` səthini əks etdirin.
- Dəyişdiriciləri və telemetriya inteqrasiyasını göstərmək üçün nümunə tətbiqləri və README-i yeniləyin.

### Telemetriya (İdarə panelləri + İxracatçılar)
- Paritet lenti (mobile_parity.json):
  - `acceleration.metal/neon/strongbox` -> {aktiv, paritet, perf_delta_pct}.
  - `perf_delta_pct` əsas CPU və GPU müqayisəsini qəbul edin.
  - `acceleration.metal.disable_reason` güzgülər `AccelerationBackendStatus.lastError`
    beləliklə, Swift avtomatlaşdırılması deaktiv edilmiş GPU-ları Rust ilə eyni sədaqətlə qeyd edə bilər
    idarə panelləri.
- CI feed (mobile_ci.json):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {cpu, metal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> İkiqat.
- İxracatçılar ölçüləri Rust benchmarklarından və ya CI qaçışlarından (məsələn, run
  `ci/xcode-swift-parity` hissəsi kimi metal/CPU mikrobench).### Konfiqurasiya düymələri və defoltlar (WP6-C)
- `AccelerationConfig` defoltları: macOS qurğularında `enable_metal = true`, CUDA funksiyası tərtib edildikdə `enable_cuda = true`, `max_gpus = None` (qapaq yoxdur). Swift `AccelerationSettings` sarğısı `connect_norito_set_acceleration_config` vasitəsilə eyni defoltları miras alır.
- Norito Merkle evristikası (GPU vs CPU): `merkle_min_leaves_gpu = 8192` ≥8192 yarpaqlı ağaclar üçün GPU heşinqinə imkan verir; backend ləğvetmələri (`merkle_min_leaves_metal`, `merkle_min_leaves_cuda`) açıq şəkildə təyin edilmədikcə eyni hədddir.
- CPU üstünlük evristikası (SHA2 ISA mövcuddur): həm AArch64 (ARMv8 SHA2), həm də x86/x86_64 (SHA-NI) üzərində CPU yolu `prefer_cpu_sha2_max_leaves_* = 32_768` buraxana qədər üstünlük təşkil edir; GPU həddinin tətbiq olunduğu yuxarıda. Bu dəyərlər `AccelerationConfig` vasitəsilə konfiqurasiya edilə bilər və yalnız benchmark sübutları ilə düzəliş edilməlidir.

## Test Strategiyası

1. **Vahid paritet testləri (Rust)**: Metal ləpələrin CPU çıxışlarına uyğun olduğundan əmin olun
   deterministik vektorlar; `cargo test -p ivm --features metal` altında işləyin.
   `crates/fastpq_prover/src/metal.rs` indi yalnız macOS üçün paritet testlərini göndərir
   Skalar istinada qarşı FFT/IFFT/LDE və Poseidon həyata keçirin.
2. **Swift tüstü qoşqu**: CPU və Metalla işləmək üçün IOS6 test qurğusunu genişləndirin
   həm emulyatorlarda, həm də StrongBox cihazlarında kodlaşdırma (Merkle/CRC64); müqayisə etmək
   nəticələr və log paritet statusu.
3. **CI**: təkan vermək üçün `norito_bridge_ios.yml`-i yeniləyin (artıq `make swift-ci`-ə zəng edir)
   artefaktlara sürətlənmə ölçüləri; qaçışın Buildkite-ı təsdiqlədiyinə əmin olun
   Qoşqun dəyişikliklərini dərc etməzdən əvvəl `ci/xcframework-smoke:<lane>:device_tag` metadata,
   və paritet/benchmark driftdə zolağı keçin.
4. **İdarəetmə panelləri**: yeni sahələr indi CLI çıxışında göstərilir. İxracatçıların istehsalını təmin edin
   tablosunu dəyişməzdən əvvəl məlumatlar.

## WP2-A Metal Shader Planı (Poseidon Boru Kəmərləri)

İlk WP2 mərhələ Poseidon Metal ləpələri üçün planlaşdırma işlərini əhatə edir
CUDA tətbiqini əks etdirir. Plan səyləri nüvələrə bölür,
ev sahibi planlaşdırması və paylaşılan daimi səhnələşdirmə, beləliklə, sonrakı iş sırf diqqət mərkəzində ola bilər
həyata keçirilməsi və sınaqdan keçirilməsi.

### Kernel Scope

1. `poseidon_permute`: `state_count` müstəqil dövlətləri dəyişdirir. Hər ip
   `STATE_CHUNK` (4 dövlət) sahibidir və istifadə edərək bütün `TOTAL_ROUNDS` iterasiyalarını həyata keçirir.
   Göndərmə zamanı mərhələli mövzu qrupu ilə paylaşılan dəyirmi sabitlər.
2. `poseidon_hash_columns`: seyrək `PoseidonColumnSlice` kataloqunu oxuyur və
   hər sütunun Merkle dostu heşinqini həyata keçirir (CPU-lara uyğun gəlir
   `PoseidonColumnBatch` düzümü). O, eyni mövzu qrupu sabit buferindən istifadə edir
   permute ləpəsi kimi, lakin `(states_per_lane * block_count)` üzərində döngələr
   Çıxışlar, beləliklə nüvə növbə təqdimatlarını amortizasiya edə bilər.
3. `poseidon_trace_fused`: iz cədvəli üçün ana/yarpaq həzmlərini hesablayır
   bir keçiddə. Birləşdirilmiş ləpə `PoseidonFusedArgs` istehlak edir, buna görə də host
   bitişik olmayan bölgələri və `leaf_offset`/`parent_offset` təsvir edə bilər və
   bütün dəyirmi/MDS cədvəllərini digər nüvələrlə paylaşır.

### Komandanın Planlaşdırılması və Host Müqavilələri- Hər bir nüvənin göndərilməsi `MetalPipelines::command_queue` vasitəsilə həyata keçirilir
  adaptiv planlaşdırıcını (hədəf ~2 ms) və növbənin fan-out nəzarətlərini tətbiq edir
  `FASTPQ_METAL_QUEUE_FANOUT` vasitəsilə məruz qalır və
  `FASTPQ_METAL_COLUMN_THRESHOLD`. `with_metal_state`-də istiləşmə yolu
  hər üç Poseidon ləpəsini qabaqda tərtib edir, belə ki, ilk göndəriş bunu etmir
  boru kəmərinin yaradılması üçün cərimə ödəyin.
- Mövzu qrupunun ölçüləri mövcud Metal FFT/LDE defoltlarını əks etdirir: hədəf budur
  Qrup başına 256 mövzudan ibarət sərt qapaq ilə hər təqdimat üçün 8,192 mövzu. The
  host aşağı güclü cihazlar üçün `states_per_lane` çarpanını aşağı sala bilər
  ətraf mühitin yığılması ləğv edilir (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  WP2-B-də əlavə edilməlidir) şeyder məntiqini dəyişdirmədən.
- Sütun quruluşu artıq FFT tərəfindən istifadə edilən eyni ikili buferli hovuzu izləyir
  boru kəmərləri. Poseidon ləpələri həmin səhnələşdirmə buferinə xam göstəriciləri qəbul edir
  və yaddaş determinizmini saxlayan qlobal yığın ayırmalarına heç vaxt toxunmayın
  CUDA hostu ilə uyğunlaşdırılmışdır.

### Paylaşılan sabitlər

- `PoseidonSnapshot` manifestində təsvir edilmişdir
  `docs/source/fastpq/poseidon_metal_shared_constants.md` indi kanonikdir
  dəyirmi sabitlər və MDS matrisi üçün mənbə. Hər iki Metal (`poseidon2.metal`)
  və CUDA (`fastpq_cuda.cu`) ləpələri manifest olduqda regenerasiya edilməlidir.
  dəyişikliklər.
- WP2-B iş vaxtında manifest oxuyan kiçik bir host yükləyicisi əlavə edəcək və
  SHA-256-nı telemetriyaya (`acceleration.poseidon_constants_sha`) yayır
  paritet tablosunda şeyder sabitlərinin dərc edilənlərə uyğun olduğunu təsdiq edə bilər
  snapshot.
- İstiləşmə zamanı biz `TOTAL_ROUNDS x STATE_WIDTH` sabitlərini a-ya köçürəcəyik
  `MTLBuffer` və hər cihaza bir dəfə yükləyin. Hər bir nüvə daha sonra məlumatları kopyalayır
  deterministik təmin edərək, onun parçasını emal etməzdən əvvəl mövzu qrupunun yaddaşına daxil edin
  hətta çoxlu komanda buferləri uçuşda işləyərkən sifariş.

### Doğrulama qarmaqları

- Vahid testləri (`cargo test -p fastpq_prover --features fastpq-gpu`) artacaq
  daxil edilmiş şeyder sabitlərini heşləşdirən və onları müqayisə edən təsdiq
  GPU qurğu dəstini yerinə yetirməzdən əvvəl manifestin SHA-sı.
- Mövcud nüvə statistikası dəyişir (`FASTPQ_METAL_TRACE_DISPATCH`,
  `FASTPQ_METAL_QUEUE_FANOUT`, növbə dərinliyi telemetriyası) tələb olunan sübuta çevrilir
  WP2 çıxışı üçün: hər bir sınaq işi planlaşdırıcının heç vaxt şərti pozmadığını sübut etməlidir
  konfiqurasiya edilmiş fan-out və əridilmiş iz nüvəsi növbəni aşağıda saxlayır
  adaptiv pəncərə.
- Swift XCFramework tüstü kəməri və Rust benchmark yarışçıları işə başlayacaq
  `acceleration.poseidon.permute_p90_ms{cpu,metal}` ixrac edir ki, WP2-D diaqram edə bilsin
  Yeni telemetriya lentlərini yenidən kəşf etmədən metal-prosessor deltaları.

## WP2-B Poseidon Manifest Yükləyicisi və Özünü Test Pariteti- `fastpq_prover::poseidon_manifest()` indi yerləşdirir və təhlil edir
  `artifacts/offline_poseidon/constants.ron`, SHA-256-nı hesablayır
  (`poseidon_manifest_sha256()`) və snapshotı CPU ilə təsdiqləyir
  hər hansı bir GPU işi başlamazdan əvvəl poseidon cədvəlləri. `build_metal_context()` qeyd edir
  isinmə zamanı həzm edin ki, telemetriya ixracatçıları dərc edə bilsinlər
  `acceleration.poseidon_constants_sha`.
- Manifest təhlilçisi uyğun olmayan genişlik/dərəcə/dairəvi sayma dəstlərini rədd edir və
  manifest MDS matrisinin skalyar icraya bərabər olmasını təmin edir, qarşısını alır
  kanonik cədvəllər bərpa edildikdə səssiz sürüşmə.
- `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs` əlavə edildi
  `poseidon2.metal`-ə daxil edilmiş Poseidon cədvəllərini təhlil edir və
  `fastpq_cuda.cu` və hər iki nüvənin tam olaraq eyni seriyalı olduğunu iddia edir
  manifest kimi sabitlər. Kimsə shader/CUDA-nı redaktə edərsə, CI indi uğursuz olur
  kanonik manifesti bərpa etmədən fayllar.
- Gələcək paritet qarmaqlar (WP2-C/D) `poseidon_manifest()`-dən yenidən istifadə edə bilər.
  sabitləri GPU buferlərinə yuvarlaqlaşdırın və Norito vasitəsilə həzmi ifşa edin
  telemetriya xəbərləri.

## WP2-C BN254 Metal Boru Kəmərləri və Paritet Testləri- **Əhatə və boşluq:** Host dispetçerləri, paritet qoşqular və `bn254_status()` canlıdır və `crates/fastpq_prover/metal/kernels/bn254.metal` indi Montgomery primitivləri üstəgəl mövzu qrupu ilə sinxronlaşdırılmış FFT/LDE dövrələrini həyata keçirir. Hər bir göndərmə mərhələli maneələri olan tək başlıq qrupunda bütöv bir sütunu işlədir, beləliklə ləpələr mərhələli manifestləri paralel olaraq həyata keçirir. Telemetriya artıq simlidir və planlaşdırıcının ləğv edilməsinə hörmət edilir ki, biz Goldilocks ləpələri üçün istifadə etdiyimiz eyni sübutla defolt-açıqlı buraxılışı təmin edə bilək.
- **Nəvə tələbləri:** ✅ Mərhələli twiddle/coset manifestlərindən təkrar istifadə edin, girişləri/çıxışları bir dəfə çevirin və hər bir sütun üzrə başlıq qrupunda bütün radix-2 mərhələlərini yerinə yetirin ki, çox dispetçer sinxronizasiyasına ehtiyacımız olmasın. Montgomery köməkçiləri FFT/LDE arasında bölüşdürülür, beləliklə yalnız döngə həndəsəsi dəyişdi.
- **Host naqilləri:** ✅ `crates/fastpq_prover/src/metal.rs` kanonik elementləri mərhələləşdirir, LDE buferini sıfırla doldurur, hər sütun üçün tək yiv qrupu seçir və qapı üçün `bn254_status()`-i ifşa edir. Telemetriya üçün əlavə host dəyişiklikləri tələb olunmur.
- **Mühafizəçilər qurun:** `fastpq.metallib` kirəmitli ləpələri göndərir, ona görə də şeyder sürüşsə, CI hələ də sürətlə uğursuz olur. İstənilən gələcək optimallaşdırma kompilyasiya vaxtı açarlarından daha çox telemetriya/xüsusiyyət qapılarının arxasında qalır.
- **Paritet qurğuları:** ✅ `bn254_parity` testləri GPU FFT/LDE çıxışlarını CPU qurğuları ilə müqayisə etməyə davam edir və indi Metal avadanlıqda canlı işləyir; yeni nüvə kodu yolları görünsə, dəyişdirilmiş manifest testlərini yadda saxlayın.
- **Telemetri və müqayisələr:** `fastpq_metal_bench` indi yayır:
  - FFT/LDE tək sütunlu partiyalar üçün hər göndəriş qrupunun genişliklərini, məntiqi ip saylarını və boru kəməri məhdudiyyətlərini ümumiləşdirən `bn254_dispatch` bloku; və
  - CPU bazası üçün `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` qeyd edən `bn254_metrics` bloku üstəgəl hansı GPU arxa ucunun işlədiyindən asılı olmayaraq.
  Qiymətləndirici sarğı hər iki xəritəni hər bükülmüş artefaktın üzərinə köçürür ki, WP2-D idarə panelləri xam əməliyyatlar massivində tərs mühəndislik yaratmadan etiketli gecikmələri/həndəsəni qəbul etsin. `FASTPQ_METAL_THREADGROUP` indi BN254 FFT/LDE göndərişlərinə də aiddir, bu da düyməni mükəmməl işləmək üçün yararlı edir. süpürür.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_73】bench

## Açıq Suallar (2027-ci ilin mayında həll edilib)1. **Metal ehtiyatının təmizlənməsi:** `warm_up_metal()` yerli ipdən yenidən istifadə edir
   `OnceCell` və indi idempotens/reqressiya testləri var
   (`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`), beləliklə tətbiqin həyat dövrü keçidləri
   sızma və ya ikiqat işə salmadan isinmə yolunu təhlükəsiz çağıra bilər.
2. **Benchmark bazası:** Metal zolaqlar CPU-nun 20%-i daxilində qalmalıdır.
   FFT/IFFT/LDE üçün baza və Poseidon CRC/Merkle köməkçiləri üçün 15% daxilində;
   xəbərdarlıq `acceleration.*_perf_delta_pct > 0.20` (yaxud itkin) olduqda işə salınmalıdır
   mobil paritet lentində. 20k iz paketində müşahidə edilən IFFT reqressiyaları
   indi WP2-D-də qeyd olunan növbənin ləğvi ilə bağlı qapalıdır.
3. **StrongBox ehtiyatı:** Swift, Android-in geri qaytarılması kitabını izləyir
   dəstək runbook-da attestasiya uğursuzluqlarının qeyd edilməsi
   (`docs/source/sdk/swift/support_playbook.md`) və avtomatik olaraq keçid
   audit qeydi ilə sənədləşdirilmiş HKDF tərəfindən dəstəklənən proqram yolu; paritet vektorları
   mövcud OA qurğuları vasitəsilə paylaşılan qalın.
4. **Telemetriya yaddaşı:** Sürətləndirmə çəkilişləri və cihaz hovuzunun sübutlarıdır
   `configs/swift/` altında arxivləşdirilmişdir (məsələn,
   `configs/swift/xcframework_device_pool_snapshot.json`) və ixracatçılar
   eyni tərtibatı əks etdirməlidir (`artifacts/swift/telemetry/acceleration/*.json`
   və ya `.prom`) beləliklə, Buildkite annotasiyaları və portal idarə panelləri
   ad-hoc qırıntı olmadan qidalanır.

## Növbəti Addımlar (Fevral 2026)

- [x] Rust: torpaq Metal host inteqrasiyası (`crates/fastpq_prover/src/metal.rs`) və
      Swift üçün nüvə interfeysini ifşa etmək; doc hand-off ilə yanaşı izlənir
      Swift körpü qeydləri.
- [x] Swift: SDK səviyyəsində sürətləndirmə parametrlərini ifşa edin (yanvar 2026-cı il hazırlanıb).
- [x] Telemetriya: `scripts/acceleration/export_prometheus.py` indi çevrilir
      `cargo xtask acceleration-state --format json` çıxışı Prometheus-ə
      mətn faylı (isteğe bağlı `--instance` etiketi ilə) beləliklə CI əməliyyatları GPU/CPU əlavə edə bilər
      aktivləşdirmə, həddlər və paritet/söndürmə səbəbləri birbaşa mətn faylına
      sifarişli qırıntı olmadan kollektorlar.
- [x] Swift QA: `scripts/acceleration/acceleration_matrix.py` çoxlu aqreqatlar
      sürətləndirmə vəziyyəti cihaz tərəfindən əsaslanan JSON və ya Markdown cədvəllərinə daxil olur
      etiket, tüstü kəmərinə deterministik “CPU vs Metal/CUDA” matrisini verir
      nümunə tətbiqi siqaretlə yanaşı yükləmək üçün. Markdown çıxışı onu əks etdirir
      Buildkite sübut formatı beləliklə idarə panelləri eyni artefaktı qəbul edə bilsin.
- [x] status.md-ni indi yeniləyin ki, `irohad` növbə/sıfır doldurma ixracatçılarını göndərir və
      env/config doğrulama testləri Metal növbənin ləğv edilməsini əhatə edir, beləliklə, WP2-D
      telemetriya + bağlamalara canlı sübutlar əlavə edilmişdir.【crates/iroha/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

Telemetriya/ixrac köməkçisi əmrləri:

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

## WP2-D Release Benchmark & Binding Notes- **20k-cərgə buraxılış ələ keçirmə:** macOS14-də yeni Metal və CPU müqayisəsi qeydə alınıb
  (arm64, zolaqlı balanslaşdırılmış parametrlər, doldurulmuş 32,768 sıra iz, iki sütun dəstəsi) və
  JSON paketini `fastpq_metal_bench_20k_release_macos14_arm64.json`-ə yoxladı.
  Etalon, əməliyyat başına ixrac vaxtları və Poseidon mikrobench sübut edir
  WP2-D yeni Metal növbə evristikası ilə əlaqəli GA keyfiyyətli artefakt var. Başlıq
  deltalar (tam cədvəl `docs/source/benchmarks.md`-də yaşayır):

  | Əməliyyat | CPU orta (ms) | Metal orta (ms) | Sürətləndirmə |
  |----------|---------------|-----------------|---------|
  | FFT (32,768 giriş) | 12.741 | 10.963 | 1,16× |
  | IFFT (32,768 giriş) | 17.499 | 25.688 | 0,68× *(reqressiya: determinizmi saxlamaq üçün növbə fan-out dayandırıldı; izləmə tənzimləmə tələb olunur)* |
  | LDE (262,144 giriş) | 68.389 | 65.701 | 1,04× |
  | Poseidon hash sütunları (524,288 giriş) | 1,728.835 | 1,447.076 | 1,19× |

  Hər bir tutma qeydi `zero_fill` vaxtlarını (33,554,432 bayt üçün 9,651 ms) və
  `poseidon_microbench` girişləri (defolt zolağı 596.229ms vs skalar 656.251ms,
  1.10× sürətləndirmə) beləliklə, tablosuna daxil olan istehlakçılar növbə təzyiqi ilə yanaşı dəyişə bilər
  əsas əməliyyatlar.
- ** Bağlamalar/sənədlər arası keçid:** `docs/source/benchmarks.md` indi istinad edir
  JSON və reproduktor əmrini buraxın, Metal növbənin ləğvi təsdiqlənir
  `iroha_config` env/manifest testləri vasitəsilə və `irohad` canlı yayımlayır
  `fastpq_metal_queue_*` ölçmələr edir ki, tablolar IFFT reqressiyaları olmadan işarələyir
  ad-hoc log kazıma. Swift-in `AccelerationSettings.runtimeState`-i ifşa edir
  eyni telemetriya yükü WP2-D-ni bağlayaraq JSON paketində göndərilir
  təkrarlana bilən qəbul bazası ilə məcburi/sənəd boşluğu.【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **IFFT növbəsinin düzəldilməsi:** Tərs FFT dəstləri indi istənilən vaxt çox növbəli göndərişi atlayır
  iş yükü ventilyasiya həddinə çətinliklə cavab verir (zolaqda balanslaşdırılmış 16 sütun
  profili), saxlayarkən yuxarıda göstərilən Metal-Vs-CPU reqressiyasını silməklə
  FFT/LDE/Poseidon üçün çox növbəli yolda böyük sütunlu iş yükləri.