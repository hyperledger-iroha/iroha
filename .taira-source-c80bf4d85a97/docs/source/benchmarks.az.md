---
lang: az
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2025-12-29T18:16:35.920013+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Müqayisə Hesabatı

Təfərrüatlı çəkilişlər və FASTPQ WP5-B tarixçəsi yaşayır
[`benchmarks/history.md`](benchmarks/history.md); qoşarkən həmin indeksdən istifadə edin
yol xəritəsi araşdırmalarına və ya SRE auditlərinə artefaktlar. İlə bərpa edin
Yeni GPU ələ keçirdikdə `python3 scripts/fastpq/update_benchmark_history.py`
və ya Poseidon torpaq təzahür edir.

## Sürətləndirmə sübut paketi

Hər bir GPU və ya qarışıq rejimli etalon tətbiq olunan sürətləndirmə parametrlərini ehtiva etməlidir
beləliklə, WP6-B/WP6-C zamanlama artefaktları ilə yanaşı konfiqurasiya paritetini sübut edə bilər.

- Hər qaçışdan əvvəl/sonra iş vaxtı şəklini çəkin:
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (insan tərəfindən oxuna bilən jurnallar üçün `--format table` istifadə edin). Bu `enable_{metal,cuda}` qeyd edir,
  Merkle hədləri, SHA-2 CPU meyl məhdudiyyətləri, aşkar edilmiş arxa uç sağlamlıq bitləri və hər hansı
  yapışqan paritet səhvləri və ya söndürmə səbəbləri.
- JSON-u bükülmüş benchmark çıxışının yanında saxlayın
  (`artifacts/fastpq_benchmarks/*.json`, `benchmarks/poseidon/*.json`, Merkle tarama
  ələ keçirmə və s.) beləliklə, rəyçilər vaxtları və konfiqurasiyanı birlikdə fərqləndirə bilər.
- Düymə tərifləri və defoltları `docs/source/config/acceleration.md`-də yaşayır; nə vaxt
  ləğvetmələr tətbiq edilir (məsələn, `ACCEL_MERKLE_MIN_LEAVES_GPU`, `ACCEL_ENABLE_CUDA`),
  Rerunsları hostlar arasında təkrarlana bilən saxlamaq üçün onları icra metadatasında qeyd edin.

## Norito Mərhələ-1 etalon (WP5-B/C)

- Komanda: `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  hər ölçüyə uyğun olaraq `benchmarks/norito_stage1/` altında JSON + Markdown yayır
  skaler vs sürətləndirilmiş struktur indeks qurucusu üçün.
- Ən son buraxılışlar (macOS aarch64, dev profili) canlı olaraq
  `benchmarks/norito_stage1/latest.{json,md}` və təzə kəsilmiş CSV
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) SIMD göstərir
  ~6–8KiB-dən sonra qazanır. GPU/paralel Mərhələ-1 indi defolt olaraq **192KiB**-dir
  işə salınma təhlükəsinin qarşısını almaq üçün kəsmə (əsr etmək üçün `NORITO_STAGE1_GPU_MIN_BYTES=<n>`).
  kiçik sənədlərdə daha böyük yüklər üçün sürətləndiricilərə imkan verir.

## Enum vs Trait Obyekt Göndərmə

- Kompilyasiya vaxtı (debug qurulması): 16.58s
- İcra müddəti (Kriteriya, aşağı daha yaxşıdır):
  - `enum`: 386 p (orta)
  - `trait_object`: 1,56 ns (orta)

Bu ölçmələr siyahıya əsaslanan göndərişi qutulu xüsusiyyət obyektinin tətbiqi ilə müqayisə edən mikrobenchmarkdan gəlir.

## Poseidon CUDA toplusu

Poseidon etalonuna (`crates/ivm/benches/bench_poseidon.rs`) indi həm tək hash dəyişdirmələri, həm də yeni toplu köməkçiləri həyata keçirən iş yükləri daxildir. Paketi aşağıdakılarla işlədin:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Kriteriya `target/criterion/poseidon*_many` altında nəticələri qeyd edəcək. GPU işçisi mövcud olduqda, JSON xülasələrini ixrac edin (məsələn, `target/criterion/**/new/benchmark.json`-i `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`-ə köçürün) (məsələn, `target/criterion/**/new/benchmark.json`-i `benchmarks/poseidon/`-ə köçürün) beləliklə, aşağı axın komandaları CPU və CUDA ölçüsünü hər bir batch ölçüsü ilə müqayisə edə bilsinlər. Xüsusi GPU zolağı fəaliyyətə başlayana qədər, etalon SIMD/CPU tətbiqinə qayıdır və hələ də toplu performans üçün faydalı reqressiya məlumatlarını təmin edir.

Təkrarlanan çəkilişlər üçün (və vaxt məlumatları ilə paritet sübutunu saxlamaq üçün) işə salın

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```hansı toxumların deterministik Poseidon2/6 partiyaları, CUDA sağlamlıq/söndürmə səbəblərini qeyd edir, yoxlayır
skalyar yola qarşı paritet və Metal ilə birlikdə əməliyyat/san + sürətləndirmə xülasələri yayır
iş vaxtı statusu (xüsusiyyət bayrağı, mövcudluq, son xəta). Yalnız CPU-ya sahib olan hostlar hələ də skalyar yazır
istinad edin və çatışmayan sürətləndiriciyə diqqət yetirin, beləliklə CI artefaktları GPU olmadan da dərc edə bilər
qaçışçı.

## FASTPQ Metal benchmark (Apple Silicon)

GPU zolağı, zolaq balanslaşdırılmış parametrlər dəsti, 20.000 məntiqi sıra (32.768-ə qədər doldurulmuş) və 16 sütun qrupu ilə macOS 14-də (arm64) `fastpq_metal_bench`-in yenilənmiş son-uca işini ələ keçirdi. Bükülmüş artefakt `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json`-də yaşayır, Metal izi `traces/fastpq_metal_trace_*_rows20000_iter5.trace` altında əvvəlki çəkilişlərlə yanaşı saxlanılır. Orta hesablanmış vaxtlar (`benchmarks.operations[*]`-dən) indi oxuyur:

| Əməliyyat | CPU orta (ms) | Metal orta (ms) | Sürətləndirmə (x) |
|-----------|---------------|-----------------|-------------|
| FFT (32,768 giriş) | 83.29 | 79.95 | 1.04 |
| IFFT (32,768 giriş) | 93.90 | 78.61 | 1.20 |
| LDE (262,144 giriş) | 669.54 | 657.67 | 1.02 |
| Poseidon hash sütunları (524,288 giriş) | 29,087,53 | 30,004,90 | 0,97 |

Müşahidələr:

- FFT/IFFT hər ikisi yenilənmiş BN254 ləpələrindən faydalanır (IFFT əvvəlki reqressiyanı ~20% təmizləyir).
- LDE paritetə ​​yaxın qalır; sıfır doldurma indi orta hesabla 18,66 ms ilə 33,554,432 doldurulmuş baytı qeyd edir, beləliklə JSON paketi növbə təsirini çəkir.
- Poseidon hashing hələ də bu aparatda CPU-ya bağlıdır; Metal yolu ən son növbə nəzarətlərini qəbul edənə qədər Poseidon mikrobench təzahürləri ilə müqayisə etməyə davam edin.
- Hər ələ indi `AccelerationSettings.runtimeState().metal.lastError` qeyd edir, icazə verir
  Mühəndislər xüsusi söndürmə səbəbi ilə CPU ehtiyatlarını şərh edirlər (siyasət keçidi,
  paritet çatışmazlığı, cihaz yoxdur) birbaşa etalon artefaktda.

Qaçışı təkrarlamaq üçün Metal ləpələri qurun və yerinə yetirin:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

`artifacts/fastpq_benchmarks/` altında yaranan JSON-u Metal izi ilə birlikdə yerinə yetirin ki, determinizm sübutları təkrarlana bilsin.

## FASTPQ CUDA avtomatlaşdırılması

CUDA hostları SM80 etalonunu bir addımda işlədə və əhatə edə bilər:

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

Köməkçi `fastpq_cuda_bench` çağırır, etiketlər/cihaz/qeydlər, fəxri adlar vasitəsilə
`--require-gpu` və (defolt olaraq) `scripts/fastpq/wrap_benchmark.py` vasitəsilə sarılır/işarələnir.
Çıxışlara xam JSON, `artifacts/fastpq_benchmarks/` altında bükülmüş paket,
və dəqiq əmrləri/env-i qeyd edən çıxışın yanında `<name>_plan.json`
Mərhələ 7 çəkilişləri GPU qaçışçıları arasında təkrarlana bilir. `--sign-output` əlavə edin və
İmza tələb olunduqda `--gpg-key <id>`; yalnız yaymaq üçün `--dry-run` istifadə edin
dəzgahı icra etmədən plan/yollar.

### GA buraxılışının çəkilməsi (macOS 14 arm64, zolaq balanslı)

WP2-D-ni təmin etmək üçün biz eyni hostda GA-hazır olan buraxılış quruluşunu da qeyd etdik
evristikanı növbəyə qoyun və onu olaraq nəşr etdi
`fastpq_metal_bench_20k_release_macos14_arm64.json`. Artefakt iki nəfəri tutur
sütun partiyaları (zolaqlı balanslaşdırılmış, 32,768 cərgəyə doldurulmuş) və Poseidon daxildir
tablosuna istehlak üçün microbench nümunələri.| Əməliyyat | CPU orta (ms) | Metal orta (ms) | Sürətləndirmə | Qeydlər |
|-----------|---------------|-----------------|---------|-------|
| FFT (32,768 giriş) | 12.741 | 10.963 | 1,16× | GPU nüvələri yenilənmiş növbə hədlərini izləyir. |
| IFFT (32,768 giriş) | 17.499 | 25.688 | 0,68× | Mühafizəkar növbə fan-out üçün izlənilən reqressiya; evristikanı tənzimləməyə davam edin. |
| LDE (262,144 giriş) | 68.389 | 65.701 | 1,04× | Sıfır doldurma qeydləri hər iki partiya üçün 9,651 ms-də 33,554,432 baytdır. |
| Poseidon hash sütunları (524,288 giriş) | 1,728.835 | 1,447.076 | 1,19× | Poseidon növbəsini dəyişdirdikdən sonra GPU nəhayət CPU-nu məğlub edir. |

JSON-a daxil edilmiş Poseidon mikrobench dəyərləri 1.10 × sürəti göstərir (standart zolaq
Beş iterasiya üzrə 596,229 ms vs skalyar 656,251 ms), beləliklə, tablolar indi diaqram edə bilər
əsas skamya ilə yanaşı hər zolaqlı təkmilləşdirmələr. Qaçışı təkrarlayın:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

Bükülmüş JSON və `FASTPQ_METAL_TRACE_CHILD=1` izlərini aşağıda qeyd edilmiş saxlayın
`artifacts/fastpq_benchmarks/` belə ki, sonrakı WP2-D/WP2-E rəyləri GA-nı fərqləndirə bilər
iş yükünü yenidən işə salmadan əvvəlki yeniləmə əməliyyatlarına qarşı çəkin.

Hər təzə `fastpq_metal_bench` ələ keçirmə indi də `bn254_metrics` blokunu yazır,
CPU üçün `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` girişlərini ifşa edir
baza və hansı GPU backendinin (Metal/CUDA) aktiv olmasından asılı olmayaraq, **və** a
`bn254_dispatch` bloku, müşahidə olunan ip qrupunun genişliklərini, məntiqi ipi qeyd edir
bir sütunlu BN254 FFT/LDE göndərişləri üçün saylar və boru kəməri limitləri. The
benchmark sarğı hər iki xəritəni `benchmarks.bn254_*`-ə köçürür, beləliklə, tablolar və
Prometheus ixracatçıları etiketli gecikmələri və həndəsəni yenidən təhlil etmədən qıra bilər
xam əməliyyatlar massivi. `FASTPQ_METAL_THREADGROUP` ləğvi indi tətbiq edilir
BN254 ləpələri də iplik qruplarının birindən təkrarlanmasını təmin edir düymə.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_73】bench

Aşağı axın tablosunu sadə saxlamaq üçün `python3 scripts/benchmarks/export_csv.py`-i işə salın
bir dəstə tutduqdan sonra. Köməkçi `poseidon_microbench_*.json`-i düzəldir
`.csv` fayllarına uyğun gəlir, beləliklə avtomatlaşdırma işləri standart və skalyar zolaqlar olmadan fərqlənə bilər
xüsusi analizatorlar.

## Poseidon mikrodəzgahı (Metal)

`fastpq_metal_bench` indi `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` altında özünü yenidən icra edir və vaxtları `benchmarks.poseidon_microbench`-ə təşviq edir. Ən son Metal çəkilişlərini `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` ilə ixrac etdik və onları `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json` vasitəsilə birləşdirdik. Aşağıdakı xülasələr `benchmarks/poseidon/` altında canlıdır:

| Xülasə | Bükülmüş paket | Defolt orta (ms) | Skalyar orta (ms) | Sürətləndirmə vs skalar | Sütunlar x dövlətlər | İterasiyalar |
|---------|----------------|-------------------|------------------|-------------------|------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1,990,49 | 1,994,53 | 1.002 | 64 x 262,144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2,167,66 | 2,152,18 | 0,993 | 64 x 262,144 | 5 |Hər iki tutma tək isinmə iterasiyası ilə hər qaçışda 262,144 vəziyyəti hash etdi (iz log2 = 12). "Defolt" zolağı tənzimlənmiş çox vəziyyətli nüvəyə uyğundur, "skalar" isə müqayisə üçün nüvəni hər zolağa bir vəziyyətə bağlayır.

## Merkle həddi süpürür

`merkle_threshold` nümunəsi (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) Metal-CPU Merkle heshing yollarını vurğulayır. Ən son AppleSilicon çəkilişi (Darwin 25.0.0 arm64, `ivm::metal_available()=true`) uyğun CSV ixracı ilə `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json`-də yaşayır. Yalnız CPU üçün macOS 14 bazası Metalsız hostlar üçün `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` altında qalır.

| Yarpaqlar | CPU ən yaxşı (ms) | Metal yaxşı (ms) | Sürətləndirmə |
|--------|---------------|-----------------|---------|
| 1,024 | 23.01 | 19.69 | 1,17× |
| 4,096 | 50.87 | 62.12 | 0,82× |
| 8,192 | 95.77 | 96.57 | 0,99× |
| 16,384 | 64.48 | 58.98 | 1,09× |
| 32,768 | 109.49 | 87.68 | 1,25× |
| 65,536 | 177.72 | 137.93 | 1,29× |

Daha böyük yarpaq sayları Metaldan (1,09–1,29×) faydalanır; kiçik vedrələr hələ də CPU-da daha sürətli işləyir, buna görə də CSV hər iki sütunu təhlil üçün saxlayır. CSV köməkçisi GPU və CPU reqressiya tablosunu uyğunlaşdırmaq üçün hər profilin yanında `metal_available` bayrağını qoruyur.

Reproduksiya mərhələləri:

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

Əgər host açıq Metal aktivləşdirməni tələb edirsə, `FASTPQ_METAL_LIB`/`FASTPQ_GPU` təyin edin və WP1-F siyasət hədlərini qrafikləşdirə bilməsi üçün CPU + GPU çəkilişlərini yoxlanılmış saxlayın.

Başsız qabıqdan işləyərkən cihazın siyahılarını qeyd etmək üçün `IVM_DEBUG_METAL_ENUM=1` və `MTLCreateSystemDefaultDevice()`-dən yan keçmək üçün `IVM_FORCE_METAL_ENUM=1` təyin edin. CLI standart Metal cihazını istəmədən **əvvəl** CoreGraphics sessiyasını qızdırır və `MTLCopyAllDevices()` sıfıra qayıtdıqda `MTLCreateSystemDefaultDevice()` səviyyəsinə düşür; əgər host hələ də heç bir cihaz bildirmirsə, çəkiliş `metal_available=false`-ni saxlayacaq (faydalı CPU bazaları `macos14_arm64_*` altında yaşayır), GPU hostları isə `FASTPQ_GPU=metal`-i aktiv saxlamalıdırlar ki, paket seçilmiş backend-i qeyd etsin.

`fastpq_metal_bench` oxşar düyməni `FASTPQ_DEBUG_METAL_ENUM=1` vasitəsilə ifşa edir, bu da arxa tərəf GPU yolunda qalıb-qalmamağa qərar verməmişdən əvvəl `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` nəticələrini çap edir. `FASTPQ_GPU=gpu` hələ də bükülmüş JSON-da `backend="none"` məlumat verdikdə onu aktivləşdirin ki, tutma paketi hostun Metal avadanlığı necə sadaladığını dəqiq qeyd etsin; qoşqu `FASTPQ_GPU=gpu` qurulduqda dərhal dayandırılır, lakin heç bir sürətləndirici aşkar edilmir, sazlama düyməsinə işarə edir ki, buraxılış paketi heç vaxt məcburi GPU-nun arxasında CPU ehtiyatını gizlətmir. çalıştırın.【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】

CSV köməkçisi `metal_available` bayrağını qoruyaraq hər profil cədvəlini (məsələn, `macos14_arm64_*.csv` və `takemiyacStudio.lan_25.0.0_arm64.csv`) yayır, beləliklə, reqressiya panelləri xüsusi analizatorlar olmadan CPU və GPU ölçmələrini qəbul edə bilər.