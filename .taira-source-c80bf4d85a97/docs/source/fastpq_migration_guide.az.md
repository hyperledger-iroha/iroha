---
lang: az
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ İstehsal Miqrasiya Bələdçisi

Bu runbook Stage6 istehsalı FASTPQ proverinin necə doğrulanacağını təsvir edir.
Bu miqrasiya planının bir hissəsi kimi deterministik yer tutucu arxa uç silindi.
O, `docs/source/fastpq_plan.md`-də mərhələli planı tamamlayır və artıq izlədiyinizi güman edir.
`status.md`-də iş sahəsinin vəziyyəti.

## Auditoriya və Əhatə dairəsi
- Təhlil və ya əsas şəbəkə mühitlərində istehsal proverini yayaraq yoxlayan operatorlar.
- İstehsal backend ilə göndəriləcək binar və ya konteynerlər yaradan mühəndisləri buraxın.
- SRE/müşahidə qrupları yeni telemetriya siqnallarının ötürülməsi və xəbərdarlıq.

Əhatə dairəsi xaricində: Kotodama müqavilə müəllifi və IVM ABI dəyişiklikləri (bax: `docs/source/nexus.md` üçün
icra modeli).

## Xüsusiyyət Matrisi
| Yol | Aktivləşdirmək üçün yük xüsusiyyətləri | Nəticə | Nə vaxt istifadə etməli |
| ---- | -------------------------------- | ------ | ----------- |
| İstehsal sübutu (standart) | _heç biri_ | FFT/LDE planlayıcısı və DEEP-FRI boru kəməri ilə Stage6 FASTPQ backend.【crates/fastpq_prover/src/backend.rs:1144】 | Bütün istehsal ikili faylları üçün defolt. |
| İsteğe bağlı GPU sürətləndirilməsi | `fastpq_prover/fastpq-gpu` | Avtomatik CPU geri qaytarılması ilə CUDA/Metal ləpələrini işə salır.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Dəstəklənən sürətləndiriciləri olan hostlar. |

## Quraşdırma Proseduru
1. **Yalnız CPU qurmaq**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   İstehsalın arxa hissəsi standart olaraq tərtib edilir; heç bir əlavə xüsusiyyət tələb olunmur.

2. **GPU ilə təchiz edilmiş quruluş (istəyə görə)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU dəstəyi quraşdırma zamanı `nvcc` ilə SM80+ CUDA alət dəsti tələb edir.【crates/fastpq_prover/Cargo.toml:11】

3. **Özünü testlər**
   ```bash
   cargo test -p fastpq_prover
   ```
   Qablaşdırmadan əvvəl Stage6 yolunu təsdiqləmək üçün bunu hər buraxılış quruluşunda bir dəfə işə salın.

### Metal alət zəncirinin hazırlanması (macOS)
1. Quraşdırmadan əvvəl Metal əmr xətti alətlərini quraşdırın: `xcode-select --install` (CLI alətləri yoxdursa) və `xcodebuild -downloadComponent MetalToolchain` GPU alətlər zəncirini gətirmək üçün. Quraşdırma skripti birbaşa `xcrun metal`/`xcrun metallib`-i işə salır və ikili fayllar olmadıqda tez uğursuz olacaq.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. Boru kəmərini CI-dən əvvəl doğrulamaq üçün siz qurma skriptini yerli olaraq əks etdirə bilərsiniz:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Bu uğur qazandıqda quraşdırma `FASTPQ_METAL_LIB=<path>` buraxır; iş vaxtı metallibi deterministik şəkildə yükləmək üçün həmin dəyəri oxuyur.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Metal alətlər silsiləsi olmadan çarpaz tərtib edərkən `FASTPQ_SKIP_GPU_BUILD=1` təyin edin; qurma xəbərdarlıq çap edir və planlayıcı CPU yolunda qalır.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Metal mövcud olmadıqda (çatışmayan çərçivə, dəstəklənməyən GPU və ya boş `FASTPQ_METAL_LIB`) qovşaqlar avtomatik olaraq CPU-ya qayıdır; qurma skripti env var-i təmizləyir və planlaşdırıcı endirməni qeyd edir.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### Buraxılış yoxlama siyahısı (Mərhələ 6)
Aşağıdakı hər bir element tamamlanana və əlavə olunana qədər FASTPQ buraxılış biletini bloklanmış saxlayın.

1. **Saniyə altı sübut göstəriciləri** — Təzə çəkilmiş `fastpq_metal_bench_*.json` və
   `benchmarks.operations` girişini təsdiqləyin, burada `operation = "lde"` (və əks olunan
   `report.operations` nümunəsi) 20000 sıra iş yükü (32768 dolğun) üçün `gpu_mean_ms ≤ 950` hesabatını verir
   sıra). Yoxlama siyahısı imzalanmadan əvvəl tavandan kənar çəkilişlər təkrar çəkiliş tələb edir.
2. **İmzalanmış manifest** — Qaçış
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   beləliklə, buraxılış bileti həm manifest, həm də onun ayrılmış imzasını daşıyır
   (`artifacts/fastpq_bench_manifest.sig`). Rəyçilər əvvəllər həzm/imza cütünü yoxlayırlar
   buraxılışı təşviq etmək.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 Matris manifest (quraşdırılmış)
   `scripts/fastpq/capture_matrix.sh` vasitəsilə) artıq 20k sıra mərtəbəni kodlayır və
   reqressiyanın sazlanması.
3. **Sübut əlavələri** — Metal etalon JSON, stdout jurnalını (və ya Alətlərin izi) yükləyin
   CUDA/Metal manifest çıxışları və buraxılış biletinin ayrılmış imzası. Yoxlama siyahısına giriş
   bütün artefaktlara üstəgəl aşağı axın auditlərini imzalamaq üçün istifadə edilən açıq açar barmaq izinə keçid etməlidir
   doğrulama addımını təkrarlaya bilər.【artifacts/fastpq_benchmarks/README.md:65】### Metal doğrulama iş axını
1. GPU-nu aktivləşdirdikdən sonra `FASTPQ_METAL_LIB` nöqtələrini `.metallib` (`echo $FASTPQ_METAL_LIB`) ilə təsdiqləyin ki, iş vaxtı onu deterministik şəkildə yükləyə bilsin.【crates/fastpq_prover/build.rs:188】
2. GPU zolaqları məcburi olaraq paritet dəstini işə salın:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Arxa uç Metal ləpələri işlədəcək və aşkarlama uğursuz olarsa, deterministik CPU geri dönüşünü qeyd edəcək.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. İdarə panelləri üçün müqayisə nümunəsini çəkin:\
   tərtib edilmiş Metal kitabxanasını tapın (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   onu `FASTPQ_METAL_LIB` vasitəsilə ixrac edin və işə salın\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  Kanonik `fastpq-lane-balanced` profili indi hər çəkilişi 32,768 sıraya (2¹⁵) çatdırır, beləliklə, JSON həm `rows`, həm də `padded_rows` və Metal LDE gecikmə müddətini daşıyır; `zero_fill` və ya növbə parametrləri GPU LDE-ni AppleM seriyalı hostlarda 950ms (<1s) hədəfindən kənara çıxararsa, çəkilişi yenidən işə salın. Nəticə JSON/logu digər buraxılış sübutları ilə birlikdə arxivləşdirin; gecəlik macOS iş axını eyni əməliyyatı yerinə yetirir və müqayisə üçün artefaktlarını yükləyir.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Yalnız Poseidon telemetriyasına ehtiyacınız olduqda (məsələn, Alətlərin izini qeyd etmək üçün) yuxarıdakı əmrə `--operation poseidon_hash_columns` əlavə edin; dəzgah hələ də `FASTPQ_GPU=gpu`-ə hörmət edəcək, `metal_dispatch_queue.poseidon` yayacaq və yeni `poseidon_profiles` blokunu daxil edəcək ki, buraxılış paketi Poseidon darboğazını açıq şəkildə sənədləşdirsin.
  Sübutlara indi `zero_fill.{bytes,ms,queue_delta}` üstəgəl `kernel_profiles` (nüvə başına) daxildir
  doluluq, təxmin edilən GB/s və müddət statistikası) beləliklə, GPU səmərəliliyinin qrafiki heç bir problem olmadan həyata keçirilə bilər.
  xam izlərin yenidən işlənməsi və `twiddle_cache` bloku (vuruş/qaçır + `before_ms`/`after_ms`)
  önbelleğe alınmış twiddle yükləmələrin qüvvədə olduğunu sübut edir. `--trace-dir` altındakı qoşqu yenidən işə salır
  `xcrun xctrace record` və
  JSON ilə birlikdə vaxt möhürülənmiş `.trace` faylını saxlayır; yenə də sifariş verə bilərsiniz
  `--trace-output` (isteğe bağlı `--trace-template` / `--trace-seconds` ilə) bir yerə çəkərkən
  fərdi yer/şablon. JSON audit üçün `metal_trace_{template,seconds,output}` qeyd edir.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Hər çəkilişdən sonra `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` işləndikdən sonra nəşr Grafana lövhəsi/xəbərdarlıq paketi (`dashboards/grafana/fastpq_acceleration.json`, `dashboards/alerts/fastpq_acceleration_rules.yml`) üçün host metadatasını (indi `metadata.metal_trace` daxil olmaqla) daşıyır. Hesabat indi hər əməliyyat üçün `speedup` obyekti (`speedup.ratio`, `speedup.delta_ms`), sarğı qaldırıcıları `zero_fill_hotspots` (bayt, gecikmə, əldə edilmiş GB/s və Metal növbə delta sayğaclarına) düzəldilir `benchmarks.kernel_summary`, `twiddle_cache` blokunu toxunulmaz saxlayır, yeni `post_tile_dispatches` blokunu/xülasəsini kopyalayır, beləliklə rəyçilər tutma zamanı çox keçidli nüvənin işlədiyini sübut edə bilsinlər və indi Poseidon mikrobench sübutlarını ümumiləşdirir xam hesabatı yenidən təhlil etmədən skalyar-defolt gecikmə. Manifest qapısı eyni bloku oxuyur və onu buraxan GPU sübut dəstlərini rədd edir, operatorları hər dəfə plitədən sonrakı yol keçildikdə və ya çəkilişləri yeniləməyə məcbur edir. səhv konfiqurasiya edilib.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】src.2x】rc.
  Poseidon2 Metal nüvəsi eyni düymələri paylaşır: `FASTPQ_METAL_POSEIDON_LANES` (32–256, ikinin səlahiyyətləri) və `FASTPQ_METAL_POSEIDON_BATCH` (hər zolaqda 1–32 vəziyyət) sizə yenidən qurmadan işə salınma enini və hər bir zolaqda işləməyə imkan verir; host hər göndərişdən əvvəl bu dəyərləri `PoseidonArgs` vasitəsilə ötürür. Defolt olaraq iş vaxtı diskret GPU-ları VRAM səviyyəli işə salmaq üçün `MTLDevice::{is_low_power,is_headless,location}`-i yoxlayır (≥48GiB bildirildikdə `256×24`, 32GiB-də `256×20`, digər hallarda isə Prometheus aşağı gücdə qalır) `256×8` (və daha köhnə 128/64 zolaqlı hissələr hər zolağa 8/6 vəziyyətə yapışır), buna görə də əksər operatorlar heç vaxt env parametrlərini əl ilə təyin etməli deyil. `fastpq_metal_bench` indi özünü `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` altında yenidən icra edir və hər iki buraxılış profilini və skalyar zolağa qarşı ölçülmüş sürəti qeyd edən `poseidon_microbench` bloku buraxır, beləliklə, buraxılış paketləri yeni nüvənin əslində kiçildiyini sübut edə bilər və Prometheus, `poseidon_pipeline` bloku beləliklə Stage7 sübutu yeni yerləşmə səviyyələri ilə yanaşı yığın dərinliyi/üst-üstə düşmə düymələrini tutur. Normal işləmələr üçün env-i təyin olunmamış buraxın; qoşqu avtomatik olaraq təkrar icranı idarə edir, uşaq ələ keçirmə işləyə bilmədikdə uğursuzluqları qeyd edir və `FASTPQ_GPU=gpu` quraşdırıldıqda dərhal çıxır, lakin heç bir GPU backend mövcud deyil, beləliklə səssiz CPU ehtiyatları heç vaxt perf-ə gizlicə girmir artefaktlar.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】Paketləyici `metal_dispatch_queue.poseidon` deltası, paylaşılan `column_staging` sayğacları və ya `poseidon_profiles`/`poseidon_microbench` dəlil blokları olmayan Poseidon çəkilişlərini rədd edir, beləliklə operatorlar uğursuzluqları sübut etmək üçün hər hansı tutma və ya yeniləməlidirlər. speedup.【scripts/fastpq/wrap_benchmark.py:732】 Panellər və ya CI deltaları üçün müstəqil JSON lazım olduqda, `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`-i işə salın; köməkçi həm bükülmüş artefaktları, həm də xam `fastpq_metal_bench*.json` çəkilişlərini qəbul edir, standart/skalyar vaxtlar, tənzimləmə metadata və qeydə alınmış sürətlənmə ilə `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` yayır.【scripts/fastpq/export_poseidon_py:1crobench.
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`-i yerinə yetirərək qaçışı tamamlayın ki, Stage6 buraxılış yoxlama siyahısı `<1 s` LDE tavanını tətbiq etsin və buraxılışla birlikdə göndərilən imzalanmış manifest/həzm paketi yaysın. bilet.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. Yayımlamadan əvvəl telemetriyanı yoxlayın: Prometheus son nöqtəsini (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) bükün və gözlənilməz `resolved="cpu"` üçün `telemetry::fastpq.execution_mode` jurnallarını yoxlayın. girişlər.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. SRE dərsliklərinin deterministik göstərici ilə uyğunlaşması üçün onu qəsdən məcbur etməklə (`FASTPQ_GPU=cpu` və ya `zk.fastpq.execution_mode = "cpu"`) prosessorun bərpa yolunu sənədləşdirin. davranış.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. Əlavə tənzimləmə: defolt olaraq host qısa izlər üçün 16 zolaq, orta üçün 32 və bir dəfə `log_len ≥ 10/14` 64/128 zolağı seçir, `log_len ≥ 18` olduqda 256-da enir və indi paylaşılan yaddaş plitəsini beş mərhələdə saxlayır, bir dəfə kiçik I1000000000000 və `log_len ≥ 18/20/22` üçün 12/14/16 mərhələləri kirəmitdən sonrakı nüvəyə işə başlamazdan əvvəl. Bu evristikanı ləğv etmək üçün yuxarıdakı addımları yerinə yetirməzdən əvvəl `FASTPQ_METAL_FFT_LANES` (8 və 256 arasında ikinin gücü) və/və ya `FASTPQ_METAL_FFT_TILE_STAGES` (1-16) ixrac edin. Həm FFT/IFFT, həm də LDE sütun toplu ölçüləri həll edilmiş mövzu qrupunun genişliyindən əldə edilir (göndərmə başına ≈2048 məntiqi mövzu, 32 sütunda bağlanır və indi domen böyüdükcə 32→16→8→4→2→1-ə enir) LDE yolu hələ də öz domen qapaqlarını tətbiq edir; `FASTPQ_METAL_FFT_COLUMNS` (1-32) deterministik FFT toplu ölçüsünü təyin etmək üçün və hostlar arasında bit-bit müqayisəsinə ehtiyacınız olduqda LDE dispetçerinə eyni ləğvi tətbiq etmək üçün `FASTPQ_METAL_LDE_COLUMNS` (1-32) təyin edin. LDE kafel dərinliyi FFT evristikasını da əks etdirir - `log₂ ≥ 18/20/22` ilə izlər geniş kəpənəkləri kirəmitdən sonrakı nüvəyə ötürməzdən əvvəl yalnız 12/10/8 paylaşılan yaddaş mərhələlərini yerinə yetirir və siz `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) vasitəsilə bu limiti ləğv edə bilərsiniz. İş vaxtı bütün dəyərləri Metal ləpə arqları vasitəsilə ötürür, dəstəklənməyən ləğvləri sıxışdırır və həll edilmiş dəyərləri qeyd edir ki, təcrübələr metallibi yenidən qurmadan təkrarlana bilsin; benchmark JSON həm həll edilmiş tənzimləməni, həm də LDE statistikası vasitəsilə əldə edilən əsas sıfır doldurma büdcəsini (`zero_fill.{bytes,ms,queue_delta}`) üzə çıxarır, beləliklə, növbə deltaları hər çəkilişlə birbaşa əlaqələndirilir və indi `column_staging` bloku əlavə edir (toplar düzəldilir, flatten_ms, gözləyin/gözləyə bilər) ikiqat tamponlu boru kəməri ilə təqdim edilmişdir. GPU sıfır doldurma telemetriyasını bildirməkdən imtina etdikdə, qoşqu indi host tərəfindəki buferin təmizlənməsindən deterministik bir vaxtı sintez edir və onu `zero_fill` blokuna yeridir, beləliklə dəlillər heç vaxt bufer olmadan göndərilməz. sahə.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. Çox növbəli göndərmə diskret Mac-lərdə avtomatikdir: `Device::is_low_power()` yalnış qaytardıqda və ya Metal cihaz Yuva/Xarici yer barədə məlumat verdikdə, host iki `MTLCommandQueue`-i işə salır, yalnız iş yükü ≥16 sütun (sütun dairəsi üzrə miqyaslı) və fan-bataşları ilə ölçülür. uzun izlər determinizmdən ödün vermədən hər iki GPU zolaqlarını məşğul saxlayır. Maşınlar arasında təkrarlana bilən çəkilişlərə ehtiyacınız olduqda `FASTPQ_METAL_QUEUE_FANOUT` (1-4 növbə) və `FASTPQ_METAL_COLUMN_THRESHOLD` (fan-outdan əvvəl minimum ümumi sütunlar) ilə siyasəti ləğv edin; paritet testləri bu ləğvləri məcbur edir ki, çox GPU-lu Mac-lar qapalı qalsın və həll edilmiş fan-out/ərəfəsində növbə dərinliyinin yanında qeyd olunur. telemetriya.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### Arxiv üçün sübut
| Artefakt | Tutmaq | Qeydlər |
|----------|---------|-------|
| `.metallib` paketi | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` və `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, ardınca `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` və `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Metal CLI/toolchain-in quraşdırıldığını və bu tapşırıq üçün deterministik kitabxana istehsal etdiyini sübut edir.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Ətraf mühitin görüntüsü | Quraşdırıldıqdan sonra `echo $FASTPQ_METAL_LIB`; buraxılış biletinizlə mütləq yolu saxlayın. | Boş çıxış Metalın əlil olduğunu bildirir; GPU zolaqlarının daşınma artefaktında mövcud olduğu dəyər sənədlərinin qeyd edilməsi.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU pariteti qeydi | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` və `backend="metal"` və ya aşağı səviyyəli xəbərdarlıqdan ibarət fraqmenti arxivləşdirin. | Quraşdırmanı təşviq etməzdən əvvəl ləpələrin işlədiyini (və ya deterministik şəkildə geri çəkildiyini) nümayiş etdirir.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Benchmark çıxışı | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; sarın və `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` vasitəsilə imzalayın. | Bükülmüş JSON qeydləri `speedup.ratio`, `speedup.delta_ms`, FFT tənzimləmə, doldurulmuş sıralar (32,768), zənginləşdirilmiş `zero_fill`/`kernel_profiles`, düzləşdirilmiş Prometheus, `metal_dispatch_queue.poseidon`/`poseidon_profiles` blokları (`--operation poseidon_hash_columns` istifadə edildikdə) və iz metadata beləliklə GPU LDE ortalaması ≤950ms, Poseidon isə <1s qalır; həm paketi, həm də yaradılan `.json.asc` imzasını buraxılış bileti ilə birlikdə saxlayın ki, tablosuna və auditorlara yenidən işə salmadan artefaktı yoxlaya bilsinlər. iş yükləri.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Bench manifest | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Hər iki GPU artefaktını təsdiq edir, LDE orta dəyəri `<1 s` tavanını pozarsa, uğursuz olur, BLAKE3/SHA-256 həzmlərini qeyd edir və imzalanmış manifest yayır, beləliklə buraxılış yoxlama siyahısı yoxlanılmadan irəliləyə bilməz. ölçülər.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA paketi | SM80 laboratoriya hostunda `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`-i işə salın, JSON-u `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`-ə bükün/imzalayın (`--label device_class=xeon-rtx-sm80` istifadə edin ki, tablolar düzgün sinfi seçsin), `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`-ə yolu əlavə edin və saxlayın. `.json`/`.asc` manifest bərpa etməzdən əvvəl Metal artefakt ilə cütləşin. Yoxlanılan `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` auditorların gözlədiyi dəqiq paket formatını göstərir.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80 |xt:10.
| Telemetriya sübut | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` üstəgəl başlanğıc zamanı buraxılan `telemetry::fastpq.execution_mode` jurnalı. | Trafikə icazə verməzdən əvvəl Prometheus/OTEL-in `device_class="<matrix>", backend="metal"` (və ya endirmə jurnalı) ifşa etdiyini təsdiqləyir.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/74:1 || Məcburi CPU qazması | `FASTPQ_GPU=cpu` və ya `zk.fastpq.execution_mode = "cpu"` ilə qısa bir partiyanı işə salın və endirmə jurnalını çəkin. | Buraxılışın ortasında geri qaytarma tələb olunarsa, SRE runbook-larını deterministik bərpa yolu ilə uyğunlaşdırır.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| İz tutma (isteğe bağlı) | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` ilə paritet testini təkrarlayın və buraxılmış göndərmə izini yadda saxlayın. | Qiymətləndirmələri təkrarlamadan sonradan profilləşdirmə rəyləri üçün yerlik/başlıq qrupu sübutlarını qoruyur.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Çoxdilli `fastpq_plan.*` faylları bu yoxlama siyahısına istinad edir, ona görə də səhnələşdirmə və istehsal operatorları eyni sübut izini izləyirlər.【docs/source/fastpq_plan.md:1】

## Təkrarlana bilən quruluşlar
Təkrarlana bilən Stage6 artefaktları yaratmaq üçün bərkidilmiş konteyner iş prosesindən istifadə edin:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Köməkçi skript `rust:1.88.0-slim-bookworm` alətlər silsiləsi şəklini (və GPU üçün `nvidia/cuda:12.2.2-devel-ubuntu22.04`) qurur, konteynırın içərisində qurmağı idarə edir və `manifest.json`, `sha256s.txt` və tərtib edilmiş ikili faylları hədəf çıxışa yazır. kataloqu.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Ətraf mühitin üstünlüyü:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – açıq-aşkar Rust bazası/teqini bağlayın.
- `FASTPQ_CUDA_IMAGE` – GPU artefaktları istehsal edərkən CUDA bazasını dəyişdirin.
- `FASTPQ_CONTAINER_RUNTIME` – xüsusi iş vaxtını məcbur etmək; default `auto` `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`-i sınayır.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – iş vaxtının avtomatik aşkarlanması üçün vergüllə ayrılmış üstünlük sırası (defolt olaraq `docker,podman,nerdctl`).

## Konfiqurasiya Yeniləmələri
1. TOML-də icra müddətinin icra rejimini təyin edin:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   Dəyər `FastpqExecutionMode` vasitəsilə təhlil edilir və başlanğıcda arxa hissəyə ötürülür.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Lazım gələrsə, işə salındıqda ləğv edin:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI, qovşaq başlamazdan əvvəl həll edilmiş konfiqurasiyanı dəyişdirir.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Tərtibatçılar ixrac etməklə konfiqurasiyalara toxunmadan aşkarlamağa müvəqqəti məcbur edə bilərlər
   Binar işə başlamazdan əvvəl `FASTPQ_GPU={auto,cpu,gpu}`; ləğvetmə qeyd olunur və boru kəməri
   hələ də həll edilmiş rejimi göstərir.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Doğrulama Yoxlama Siyahısı
1. **Başlanğıc qeydləri**
   - `telemetry::fastpq.execution_mode` hədəfindən `FASTPQ execution mode resolved` gözləyin
     `requested`, `resolved` və `backend` etiketləri.【crates/fastpq_prover/src/backend.rs:208】
   - Avtomatik GPU aşkarlanmasında `fastpq::planner`-dən ikincil qeyd son zolağı bildirir.
   - Metallib uğurla yükləndiyi zaman `backend="metal"` səthi metaldan ibarətdir; kompilyasiya və ya yükləmə uğursuz olarsa, quraşdırma skripti xəbərdarlıq edir, `FASTPQ_METAL_LIB`-i təmizləyir və planlayıcı işləmədən əvvəl `GPU acceleration unavailable`-i qeyd edir. CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover.2. **Prometheus göstəriciləri**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Sayğac `record_fastpq_execution_mode` vasitəsilə artırılır (indi ilə etiketlənir
   `{device_class,chip_family,gpu_kind}`) qovşaq öz icrasını həll etdikdə
   rejim.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Metal örtüyü üçün təsdiqləyin
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     yerləşdirmə panelləri ilə yanaşı artımlar.【crates/iroha_telemetry/src/metrics.rs:5397】
   - `irohad --features fastpq-gpu` ilə tərtib edilmiş macOS qovşaqları əlavə olaraq ifşa olunur
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     və
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` beləliklə Stage7 idarə panelləri
     canlı Prometheus sıyrıntılarından vəzifə dövrünü izləyə və növbə boşluğunu izləyə bilər.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Telemetriya ixracı**
   - OTEL konstruksiyaları eyni etiketlərlə `fastpq.execution_mode_resolutions_total` buraxır; təmin edin
     tablosuna və ya siqnallar GPU aktiv olmalıdır zaman gözlənilməz `resolved="cpu"` üçün baxın.

4. **Sağlamlığı sübut et/yoxla**
   - `iroha_cli` və ya inteqrasiya kəməri vasitəsilə kiçik bir partiyanı işə salın və sübutları təsdiqləyin
     eyni parametrlərlə tərtib edilmiş peer.

## Problemlərin aradan qaldırılması
- **Həll edilmiş rejim GPU hostlarında CPU olaraq qalır** — binarın qurulduğunu yoxlayın
  `fastpq_prover/fastpq-gpu`, CUDA kitabxanaları yükləyici yolundadır və `FASTPQ_GPU` məcbur etmir
  `cpu`.
- **Apple Silicon-da metal əlçatmaz** — CLI alətlərinin quraşdırıldığını yoxlayın (`xcode-select --install`), `xcodebuild -downloadComponent MetalToolchain`-i yenidən işə salın və quruluşun boş olmayan `FASTPQ_METAL_LIB` yolunu yaratdığından əmin olun; boş və ya əskik dəyər dizaynla arxa ucunu qeyri-aktiv edir.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` səhvləri** — həm sübut edənin, həm də yoxlayıcının eyni kanonik kataloqdan istifadə etməsinə əmin olun
  `fastpq_isi` tərəfindən yayılır; `Error::UnknownParameter` kimi səth uyğunsuzluğu.【crates/fastpq_prover/src/proof.rs:133】
- **Gözlənilməz CPU bərpası** — `cargo tree -p fastpq_prover --features`-i yoxlayın və
  GPU quruluşlarında `fastpq_prover/fastpq-gpu` olduğunu təsdiqləyin; `nvcc`/CUDA kitabxanalarının axtarış yolunda olduğunu yoxlayın.
- **Telemetriya sayğacı yoxdur** — qovşağın `--features telemetry` ilə işə salındığını yoxlayın (defolt)
  və OTEL ixracına (əgər aktivdirsə) metrik boru kəməri daxildir.【crates/iroha_telemetry/src/metrics.rs:8887】

## Geri Qaytarma Proseduru
Deterministik yertutan arxa ucu silindi. Əgər reqressiya geri çəkilməyi tələb edirsə,
əvvəllər məlum olan yaxşı buraxılış artefaktlarını yenidən yerləşdirin və 6-cı Mərhələni yenidən nəşr etməzdən əvvəl araşdırın
ikililər. Dəyişikliklərin idarə edilməsi qərarını sənədləşdirin və irəliləmənin yalnız sonra tamamlanmasını təmin edin
reqressiya başa düşülür.

3. `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}`-in gözlənilənləri əks etdirməsini təmin etmək üçün telemetriyaya nəzarət edin
   yer tutucunun icrası.

## Hardware Baseline
| Profil | CPU | GPU | Qeydlər |
| ------- | --- | --- | ----- |
| İstinad (Mərhələ6) | AMD EPYC7B12 (32 nüvə), 256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000 sıra sintetik partiyalar ≤1000ms tamamlamalıdır.【docs/source/fastpq_plan.md:131】 |
| Yalnız CPU | ≥32 fiziki nüvə, AVX2 | – | 20000 sıra üçün ~0,9-1,2s gözləyin; determinizm üçün `execution_mode = "cpu"` saxlayın. |## Reqressiya Testləri
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU hostlarında)
- İsteğe bağlı qızıl armatur yoxlanışı:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Bu yoxlama siyahısından hər hansı sapmaları əməliyyat kitabçanızda sənədləşdirin və sonra `status.md`-i yeniləyin.
köçürmə pəncərəsi tamamlanır.