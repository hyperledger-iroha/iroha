---
lang: az
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:53:05.148398+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FASTPQ Rollout Playbook (Mərhələ 7-3)

Bu oyun kitabı Stage7-3 yol xəritəsi tələbini həyata keçirir: hər donanma təkmilləşdirməsi
FASTPQ GPU icrasına imkan verən, təkrarlana bilən benchmark manifestini əlavə etməlidir,
qoşalaşmış Grafana sübutu və sənədləşdirilmiş geri dönmə qazması. Tamamlayır
`docs/source/fastpq_plan.md` (hədəflər/arxitektura) və
Fokuslanaraq `docs/source/fastpq_migration_guide.md` (qovşaq səviyyəsində təkmilləşdirmə addımları).
operatorla üzbəüz buraxılış yoxlama siyahısında.

## Əhatə dairəsi və rollar

- **Release Engineering / SRE:** öz benchmark çəkilişləri, manifest imzalanması və
  təqdimat təsdiqindən əvvəl tablosunun ixracı.
- **Ops Guild:** mərhələli çıxışlar həyata keçirir, geri çəkilmə məşqlərini qeyd edir və saxlayır
  `artifacts/fastpq_rollouts/<timestamp>/` altında artefakt paketi.
- **İdarəetmə / Uyğunluq:** sübutların hər dəyişikliyi müşayiət etdiyini yoxlayır
  donanma üçün FASTPQ defoltunun dəyişdirilməsindən əvvəl sorğu.

## Sübut Paketi Tələbləri

Hər təqdimatda aşağıdakı artefaktlar olmalıdır. Bütün faylları əlavə edin
buraxılış/təkmilləşdirmə biletinə və paketi içəridə saxlayın
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Artefakt | Məqsəd | Necə istehsal |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` | Kanonik 20000 cərgəli iş yükünün `<1 s` LDE tavanı altında qaldığını sübut edir və hər bükülmüş meyar üçün heşləri qeyd edir.| Metal/CUDA qaçışlarını çəkin, onları sarın, sonra işə salın:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \`03`  --signing-key secrets/fastpq_bench.ed25519 \`0 |
| Bükülmüş göstəricilər (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | Host metadatasını, cərgə istifadə sübutunu, sıfır doldurma nöqtələrini, Poseidon mikrobench xülasəsini və tablosuna/xəbərdarlıqlarına görə istifadə edilən nüvə statistikasını əldə edin.| `fastpq_metal_bench` / `fastpq_cuda_bench`-i işə salın, sonra xam JSON-u bükün:`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`Prometheus üçün qapaq (müvafiq şahid/kazıma fayllarında `--row-usage` və `--poseidon-metrics` nöqtəsi). Köməkçi süzülmüş `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` nümunələrini daxil edir ki, WP2-E.6 sübutları Metal və CUDA arasında eyni olsun. Müstəqil Poseidon mikrobench xülasəsinə ehtiyacınız olduqda `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` istifadə edin (bükülmüş və ya xam girişlər dəstəklənir). |
|  |  | **Stage7 etiket tələbi:** `wrap_benchmark.py`, nəticədə yaranan `metadata.labels` bölməsində həm `device_class`, həm də `gpu_kind` olmadıqda, indi uğursuz olur. Avtomatik aşkarlama onları müəyyən edə bilməyəndə (məsələn, ayrılmış CI node-a sararkən), `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete` kimi açıq-aşkar ləğvetmələri keçin. |
|  |  | **Sürətləndirici telemetriya:** sarğı həmçinin defolt olaraq `cargo xtask acceleration-state --format json`-i çəkir, bükülmüş meyarın yanında `<bundle>.accel.json` və `<bundle>.accel.prom` yazır (`--accel-*` və ya I0850X bayraqları ilə ləğv edin). Tutma matrisi bu faylları donanmanın idarə panelləri üçün `acceleration_matrix.{json,md}` qurmaq üçün istifadə edir. |
| Grafana ixrac | Yayımlanma pəncərəsi üçün qəbul edilmiş telemetriya və xəbərdarlıq annotasiyalarını sübut edir.| `fastpq-acceleration` tablosunu ixrac edin:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`İxracdan əvvəl lövhəyə şərh yazın. Buraxılış boru kəməri bunu avtomatik olaraq `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` vasitəsilə edə bilər (token `GRAFANA_TOKEN` vasitəsilə verilir). |
| Xəbərdarlıq snapshot | Yayımlanmanı qoruyan xəbərdarlıq qaydalarını tutur.| `dashboards/alerts/fastpq_acceleration_rules.yml`-i (və `tests/` qurğusunu) paketə kopyalayın ki, rəyçilər `promtool test rules …`-i yenidən işə sala bilsinlər. |
| Geriyə qazma jurnalı | Nümayiş edir ki, operatorlar CPU-nun məcburi geri qaytarılmasını və telemetriya təsdiqini sınaqdan keçiriblər.| [Geri Qaytarma Matkapları](#rollback-drills) və konsol qeydlərini (`rollback_drill.log`) üstəgəl ortaya çıxan Prometheus qırıntısını (`metrics_rollback.prom`) saxlayın. || `row_usage/fastpq_row_usage_<date>.json` | TF-5-in CI və idarə panellərində izlədiyi ExecWitness FASTPQ sıra bölgüsünü qeyd edir.| Torii-dən yeni şahidi endirin, onu `iroha_cli audit witness --decode exec.witness` vasitəsilə deşifrə edin (istəyə görə gözlənilən parametr dəstini təsdiqləmək üçün `--fastpq-parameter fastpq-lane-balanced` əlavə edin; FASTPQ qrupları defolt olaraq buraxır) və `row_usage` JSON0400-a kopyalayın. Fayl adlarını vaxt möhürü ilə saxlayın ki, rəyçilər onları buraxılış bileti ilə əlaqələndirə bilsinlər və `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (və ya `make check-fastpq-rollout`) işə salın ki, Mərhələ7-3 qapısı hər bir partiyanın selektorun sayıldığını və `transfer_ratio = transfer_rows / total_rows` sübut əlavə etməzdən əvvəl reklam etdiyini yoxlayır. |

> **İpucu:** `artifacts/fastpq_rollouts/README.md` üstünlük verilən adlandırmanı sənədləşdirir
> sxem (`<stamp>/<fleet>/<lane>`) və tələb olunan sübut faylları. The
> `<stamp>` qovluğu `YYYYMMDDThhmmZ` kodlamalıdır ki, artefaktlar çeşidlənə bilsin
> biletlərlə məsləhətləşmədən.

## Sübut Yaratma Yoxlama Siyahısı1. **GPU göstəricilərini çəkin.**
   - Kanonik iş yükünü (20000 məntiqi sıra, 32768 doldurulmuş sətir) vasitəsilə icra edin
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - `--row-usage <decoded witness>` istifadə edərək nəticəni `scripts/fastpq/wrap_benchmark.py` ilə sarın ki, paket GPU telemetriyası ilə yanaşı qadcet sübutunu daşısın. `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output`-i keçin ki, ya sürətləndirici hədəfi keçərsə və ya Poseidon növbəsi/profil telemetriyası çatışmırsa və ayrılmış imza yaratmaq üçün sarğı sürətlə sıradan çıxsın.
   - CUDA hostunda təkrarlayın ki, manifest hər iki GPU ailəsini ehtiva etsin.
   - `benchmarks.metal_dispatch_queue` və ya ** çıxarmayın
     `benchmarks.zero_fill_hotspots` bükülmüş JSON-dan bloklanır. CI qapısı
     (`ci/check_fastpq_rollout.sh`) indi həmin sahələri oxuyur və növbə zamanı uğursuz olur
     boşluq bir yuvadan aşağı düşür və ya hər hansı LDE hotspot `mean_ms > bildirdikdə
     0.40ms`, Stage7 telemetriya mühafizəsini avtomatik tətbiq edir.
2. **Manifesti yaradın.** `cargo xtask fastpq-bench-manifest …` kimi istifadə edin
   cədvəldə göstərilir. `fastpq_bench_manifest.json`-i buraxılış paketində saxlayın.
3. **Export Grafana.**
   - Açılan pəncərə ilə `FASTPQ Acceleration Overview` lövhəsinə şərh yazın,
     müvafiq Grafana panel ID-lərinə keçid.
   - JSON tablosunu Grafana API vasitəsilə ixrac edin (yuxarıdakı əmr) və daxil edin
     `annotations` bölməsini nəzərdən keçirənlər qəbul əyrilərini uyğunlaşdıra bilsinlər
     mərhələli buraxılış.
4. **Snapshot xəbərdarlıqları.** İstifadə olunan dəqiq xəbərdarlıq qaydalarını (`dashboards/alerts/…`) kopyalayın
   paketə təqdim etməklə. Prometheus qaydaları ləğv edilibsə, daxil edin
   üstələmə fərqi.
5. **Prometheus/OTEL qırıntısı.** Hər birindən `fastpq_execution_mode_total{device_class="<matrix>"}` çəkin
   ev sahibi (səhnədən əvvəl və sonra) üstəgəl OTEL sayğacı
   `fastpq.execution_mode_resolutions_total` və qoşalaşmış
   `telemetry::fastpq.execution_mode` jurnal qeydləri. Bu əsərlər bunu sübut edir
   GPU-nun qəbulu sabitdir və bu məcburi CPU ehtiyatları hələ də telemetriya yayır.
6. **Arxiv sətir-istifadə telemetriyası.** ExecWitness-in şifrəsini açdıqdan sonra
   rollout, paketdə `row_usage/` altında nəticələnən JSON-u buraxın. CI
   helper (`ci/check_fastpq_row_usage.sh`) bu anlıq görüntüləri ilə müqayisə edir
   kanonik baza xətləri və `ci/check_fastpq_rollout.sh` indi hər birini tələb edir
   TF-5 sübutunu əlavə etmək üçün ən azı bir `row_usage` faylını göndərmək üçün paket
   buraxılış biletinə qədər.

## Mərhələli Yayım axını

Hər donanma üçün üç deterministik mərhələdən istifadə edin. Yalnız çıxışdan sonra irəliləyin
hər bir mərhələdə meyarlar təmin edilir və sübutlar paketində sənədləşdirilir.| Faza | Əhatə dairəsi | Çıxış meyarları | Qoşmalar |
|-------|-------|---------------|-------------|
| Pilot (P1) | 1 nəzarət müstəvisi + 1 məlumat müstəvisi node hər bölgə | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` 48 saat üçün ≥90%, sıfır Alertmanager insidentləri və keçən geri dönmə qazması. | Hər iki hostdan paket (dəzgah JSON-ları, pilot annotasiya ilə Grafana ixracı, geri çəkilmə qeydləri). |
| eniş (P2) | Doğrulayıcıların ≥50%-i və hər klaster üçün ən azı bir arxiv zolağı | GPU icrası 5 gün davam etdi, ən çox 1 endirmə sıçrayışı >10 dəqiqə və Prometheus sayğacları 60 saniyə ərzində ehtiyatların geri qaytarılması xəbərdarlığını sübut edir. | Ramp annotasiyasını göstərən yenilənmiş Grafana ixracı, Prometheus sıyrılma fərqləri, Alertmanager ekran görüntüsü/loqu. |
| Defolt (P3) | Qalan qovşaqlar; FASTPQ `iroha_config` |-də defolt olaraq qeyd edildi İmzalanmış dəzgah manifesti + son qəbul əyrisinə istinad edən Grafana ixracı və konfiqurasiya keçidini nümayiş etdirən sənədləşdirilmiş geri dönmə qazması. | Yekun manifest, Grafana JSON, geri qaytarma jurnalı, konfiqurasiya dəyişikliyinin nəzərdən keçirilməsinə bilet arayışı. |

Yayım biletində hər bir təşviqat addımını sənədləşdirin və birbaşa linkə keçin
`grafana_fastpq_acceleration.json` annotasiyaları beləliklə, rəyçiləri əlaqələndirə bilsinlər
dəlillərlə vaxt qrafiki.

## Geri Qaytarma Dəlilləri

Hər təqdimetmə mərhələsi geri çəkilmə məşqini əhatə etməlidir:

1. Hər klaster üçün bir qovşaq seçin və cari göstəriciləri qeyd edin:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Konfiqurasiya düyməsindən istifadə edərək CPU rejimini 10 dəqiqə məcbur edin
   (`zk.fastpq.execution_mode = "cpu"`) və ya ətraf mühitin ləğvi:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Endirmə jurnalını təsdiq edin
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) və sürtün
   sayğac artımlarını göstərmək üçün yenidən Prometheus son nöqtəsi.
4. GPU rejimini bərpa edin, `telemetry::fastpq.execution_mode`-in indi hesabat verdiyini yoxlayın
   `resolved="metal"` (və ya qeyri-metal zolaqlar üçün `resolved="cuda"/"opencl"`),
   Prometheus qırıntısının həm CPU, həm də GPU nümunələrini ehtiva etdiyini təsdiqləyin
   `fastpq_execution_mode_total{backend=…}` və keçən vaxtı qeyd edin
   aşkarlama/təmizləmə.
5. Qabıq transkriptlərini, ölçüləri və operator təsdiqlərini olaraq saxlayın
   `rollback_drill.log` və `metrics_rollback.prom` buraxılış paketində. Bunlar
   fayllar tam endirmə + bərpa dövrünü təsvir etməlidir, çünki
   `ci/check_fastpq_rollout.sh` indi logda GPU olmadıqda uğursuz olur
   bərpa xətti və ya ölçülər snapshot ya CPU və ya GPU sayğaclarını buraxır.

Bu qeydlər sübut edir ki, hər klaster zərif şəkildə pisləşə bilər və SRE komandaları
GPU sürücüləri və ya ləpələri reqress olarsa, deterministik şəkildə necə geri çəkiləcəyini bilirsiniz.

## Qarışıq rejimli ehtiyat sübut (WP2-E.6)

Hər dəfə hosta GPU FFT/LDE, lakin CPU Poseidon hashing ehtiyacı olduqda (Stage7 <900ms-ə görə)
tələb), aşağıdakı artefaktları standart geri qaytarma qeydləri ilə birlikdə yığın:1. **Konfiqurasiya fərqi.** Ayarlayan yerli host-lokal ləğvi yoxlayın (və ya əlavə edin)
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) ayrılarkən
   `zk.fastpq.execution_mode` toxunulmayıb. Yamağı adlandırın
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Poseidon sayğacının sıyrılması.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   Çəkilişdə `path="cpu_forced"` işarəsi ilə kilidləmə addımında artım göstərilməlidir.
   Həmin cihaz sinfi üçün GPU FFT/LDE sayğacı. Qaytardıqdan sonra ikinci bir cızıq çəkin
   GPU rejiminə qayıdın ki, rəyçilər `path="gpu"` sıra davamını görə bilsinlər.

   Nəticə faylı `wrap_benchmark.py --poseidon-metrics …`-ə ötürün ki, bükülmüş benchmark `poseidon_metrics` bölməsində eyni sayğacları qeyd etsin; bu, eyni iş prosesində Metal və CUDA buraxılışlarını saxlayır və ayrıca kazıma fayllarını açmadan ehtiyat sübutlarını yoxlanıla bilir.
3. **Girişdən çıxarış.** Bunu sübut edən `telemetry::fastpq.poseidon` qeydlərini kopyalayın.
   həlledici CPU-ya (`cpu_forced`) çevrildi
   `poseidon_fallback.log`, zaman ştamplarını saxlayır ki, Alertmanager vaxt qrafikləri ola bilsin
   konfiqurasiya dəyişikliyi ilə əlaqələndirilir.

CI bu gün növbə/sıfır doldurma yoxlamalarını tətbiq edir; qarışıq rejimli darvaza endikdən sonra,
`ci/check_fastpq_rollout.sh` də israr edəcək ki, hər hansı bir paketi ehtiva edir
`poseidon_fallback.patch` uyğun gələn `metrics_poseidon.prom` şəklini göndərir.
Bu iş axınının ardınca WP2-E.6 geri qaytarma siyasəti yoxlanıla bilər və ona bağlıdır
defolt olaraq işə salınma zamanı istifadə edilən eyni sübut toplayıcıları.

## Hesabat və Avtomatlaşdırma

- Bütün `artifacts/fastpq_rollouts/<stamp>/` kataloqunu
  bileti buraxın və buraxılış bağlandıqdan sonra ona `status.md`-dən istinad edin.
- `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`-i işə salın (vasitəsilə
  `promtool`) CI daxilində xəbərdarlıq paketlərinin hələ də buraxılışla birlikdə olmasını təmin etmək üçün
  tərtib etmək.
- Paketi `ci/check_fastpq_rollout.sh` (və ya
  `make check-fastpq-rollout`) və istədiyiniz zaman `FASTPQ_ROLLOUT_BUNDLE=<path>` keçir
  tək bir yayımı hədəfləmək istəyirəm. CI eyni skripti vasitəsilə çağırır
  `.github/workflows/fastpq-rollout.yml`, buna görə də itkin artefaktlar a
  buraxılış bileti bağlana bilər. Buraxılış kəməri təsdiqlənmiş paketləri arxivləşdirə bilər
  keçərək imzalanmış manifestlərin yanında
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>`-dən
  `scripts/run_release_pipeline.py`; köməkçi təkrar qaçır
  `ci/check_fastpq_rollout.sh` (`--skip-fastpq-rollout-check` təyin edilmədikdə) və
  kataloq ağacını `artifacts/releases/<version>/fastpq_rollouts/…`-ə kopyalayır.
  Bu qapının bir hissəsi olaraq skript Stage7 növbə dərinliyini və sıfır doldurmağı tətbiq edir
  büdcələri oxumaqla `benchmarks.metal_dispatch_queue` və
  Hər bir `metal` JSON dəzgahından `benchmarks.zero_fill_hotspots`.

Bu oyun kitabına riayət etməklə biz deterministik qəbulu nümayiş etdirə bilərik, a
hər buraxılış üçün tək dəlil paketi və geri qaytarma təlimlərini birlikdə yoxlayın
imzalanmış meyar özünü göstərir.