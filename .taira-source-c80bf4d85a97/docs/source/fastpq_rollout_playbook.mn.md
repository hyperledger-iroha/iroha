---
lang: mn
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

# FASTPQ Rollout Playbook (7-3-р шат)

Энэхүү тоглоомын ном нь 7-3-р шатны замын зураглалын шаардлагыг хэрэгжүүлдэг: флотын шинэчлэл бүр
FASTPQ GPU гүйцэтгэлийг идэвхжүүлдэг нь дахин давтагдах жишиг манифест хавсаргах ёстой,
хосолсон Grafana нотлох баримт, баримтжуулсан буцаах өрөмдлөг. Энэ нь нөхөж өгдөг
`docs/source/fastpq_plan.md` (зорилтот/архитектур) болон
`docs/source/fastpq_migration_guide.md` (зангилааны түвшний шинэчлэлтийн алхамууд) анхаарлаа төвлөрүүлэх замаар
операторын өмнө танилцуулах шалгах хуудас дээр.

## Хамрах хүрээ ба үүрэг

- **Release Engineering / SRE:** өөрийн жишиг зураг авалт, манифест гарын үсэг, болон
  нэвтрүүлэх зөвшөөрөл авахаас өмнө хяналтын самбарын экспорт.
- **Ops Guild:** үе шаттайгаар танилцуулж, буцаах сургуулилтуудыг бичиж, хадгалдаг.
  `artifacts/fastpq_rollouts/<timestamp>/` доорх олдворын багц.
- **Засаглал / Дагаж мөрдөх:** өөрчлөлт бүрийг нотлох баримт дагалддаг гэдгийг баталгаажуулдаг
  флотын хувьд FASTPQ өгөгдмөл тохиргоог сэлгэхээс өмнө хүсэлт гаргах.

## Нотлох баримтын багцад тавигдах шаардлага

Өргөтгөсөн танилцуулга бүр дараах олдворуудыг агуулсан байх ёстой. Бүх файлыг хавсаргана уу
гаргах/шинэчлэх тасалбар руу илгээж, багцыг дотор нь байлга
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Олдвор | Зорилго | Хэрхэн үйлдвэрлэх вэ |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` | Каноник 20000 мөрийн ажлын ачаалал нь `<1 s` LDE таазны дор байдгийг нотолж, боосон жишиг бүрийн хэшийг бүртгэдэг.| Металл/CUDA гүйлтийг барьж аваад, боож, дараа нь ажиллуулна уу:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \`003 `cargo xtask fastpq-bench-manifest \`
| Боодолтой жишиг (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | Хост мета өгөгдөл, мөр ашиглалтын нотолгоо, тэг дүүргэх цэгүүд, Poseidon микробенчийн хураангуй, хяналтын самбар/сануултанд ашигладаг цөмийн статистикийг аваарай.| `fastpq_metal_bench` / `fastpq_cuda_bench`-г ажиллуулж, дараа нь түүхий JSON-г боож өгнө үү:`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`Prometheus-д зориулсан таг (холбогдох гэрч / хусах файл дээрх `--row-usage` ба `--poseidon-metrics` цэгүүд). Туслагч нь шүүсэн `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` дээжийг суулгасан тул WP2-E.6 нотолгоо нь Метал болон CUDA-д ижил байна. Бие даасан Poseidon microbench хураангуй хэрэгтэй үед `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` ашиглана уу (боодолтой эсвэл түүхий оролтыг дэмждэг). |
|  |  | **Stage7 шошгоны шаардлага:** `wrap_benchmark.py`-ийн үр дүнд үүссэн `metadata.labels` хэсэгт `device_class` болон `gpu_kind` хоёуланг нь агуулаагүй л бол одоо амжилтгүй болно. Автомат илрүүлэлт нь тэдгээрийг гаргаж чадахгүй үед (жишээ нь, салангид CI зангилаа дээр ороох үед) `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete` гэх мэт тодорхой дарж бичнэ үү. |
|  |  | **Хурдатгалын телеметр:** боодол нь анхдагчаар `cargo xtask acceleration-state --format json`-г авч, ороосон жишиг үзүүлэлтийн хажууд `<bundle>.accel.json` болон `<bundle>.accel.prom` гэж бичдэг (`--accel-*` эсвэл I1006000-аар дарж бичнэ). Баривчлах матриц нь эдгээр файлуудыг флотын хяналтын самбарт зориулж `acceleration_matrix.{json,md}` бүтээхэд ашигладаг. |
| Grafana экспорт | Нэвтрүүлэх цонхны хувьд үрчлүүлэх телеметр болон дохиоллын тэмдэглэгээг нотолж байна.| `fastpq-acceleration` хяналтын самбарыг экспортлох:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`Экспорт хийхээс өмнө самбарт тайлбар хийнэ үү. Суллах дамжуулах хоолой нь үүнийг `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (`GRAFANA_TOKEN`-ээр хангагдсан жетон)-ээр дамжуулан автоматаар хийх боломжтой. |
| Анхааруулга | Нэвтрүүлэлтийг хамгаалсан сэрэмжлүүлгийн дүрмийг баримтална.| `dashboards/alerts/fastpq_acceleration_rules.yml` (болон `tests/` бэхэлгээг) багц руу хуулна уу, ингэснээр тоймчид `promtool test rules …`-г дахин ажиллуулах боломжтой. |
| Буцах өрмийн бүртгэл | Операторууд CPU-ийн албадан уналт болон телеметрийн мэдэгдлийг давтсан болохыг харуулж байна.| [Rollback Drills](#rollback-drills) дээрх процедурыг ашиглаж, консолын бүртгэлийг (`rollback_drill.log`) нэмээд үүссэн Prometheus хусах (`metrics_rollback.prom`) дээр хадгална уу. || `row_usage/fastpq_row_usage_<date>.json` | CI болон хяналтын самбарт TF-5-ын дагаж мөрддөг ExecWitness FASTPQ мөрийн хуваарилалтыг бүртгэдэг.| Torii-ээс шинэ гэрчийг татаж аваад `iroha_cli audit witness --decode exec.witness`-ээр тайлж (заавал `--fastpq-parameter fastpq-lane-balanced`-г нэмж хүлээгдэж буй параметрийн багцыг баталгаажуулна уу; FASTPQ багцууд нь анхдагчаар ялгардаг), `row_usage` JSON04073X руу хуулна уу. Шүүгчид тэдгээрийг танилцуулах тасалбартай уялдуулахын тулд файлын нэрийг цагийн тамгатай байлгаж, `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (эсвэл `make check-fastpq-rollout`) ажиллуулж, 7-3-р шатны хаалга нь багц бүр сонгогчийг сурталчилж байгааг баталгаажуулж, нотлох баримтыг хавсаргахаас өмнө `transfer_ratio = transfer_rows / total_rows` баталгаажуулна. |

> **Зөвлөгөө:** `artifacts/fastpq_rollouts/README.md` сонгосон нэрийг баримтжуулна
> схем (`<stamp>/<fleet>/<lane>`) болон шаардлагатай нотлох баримтын файлууд. The
> `<stamp>` хавтас нь `YYYYMMDDThhmmZ` кодлох ёстой тул олдворууд эрэмбэлэх боломжтой хэвээр байна
> тасалбарын талаар зөвлөлдөхгүйгээр.

## Нотлох баримт бүрдүүлэх хяналтын хуудас1. **GPU-н жишиг үзүүлэлтүүдийг авах.**
   - Каноник ажлын ачааллыг (20000 логик мөр, 32768 жийргэвчтэй мөр) ашиглан ажиллуулна.
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - Үр дүнг `scripts/fastpq/wrap_benchmark.py` ашиглан `--row-usage <decoded witness>` ашиглан боож, багц нь GPU телеметрийн хажууд гаджетын нотолгоог авчрах болно. `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output`-ийг дамжуулж, хурдасгуурын аль нэг нь зорилтот хэмжээнээс хэтэрсэн эсвэл Посейдоны дараалал/профайлын телеметр байхгүй бол боодол хурдан бүтэлгүйтэж, салангид гарын үсэг үүсгэх болно.
   - Манифест нь GPU гэр бүлийг хоёуланг нь агуулсан байхаар CUDA хост дээр давтана.
   - `benchmarks.metal_dispatch_queue` эсвэл **ж болохгүй**
     `benchmarks.zero_fill_hotspots` ороосон JSON-н блокууд. CI хаалга
     (`ci/check_fastpq_rollout.sh`) одоо эдгээр талбаруудыг уншиж, дараалалд ороход амжилтгүй боллоо
     Ямар нэгэн LDE сүлжээний цэг `mean_ms > гэж мэдээлэх үед толгойн зай нэг слотоос доош буурдаг
     0.40ms`, Stage7 телеметрийн хамгаалалтыг автоматаар хэрэгжүүлдэг.
2. **Манифест үүсгэх.** `cargo xtask fastpq-bench-manifest …`-г дараах байдлаар ашиглана уу.
   хүснэгтэд үзүүлэв. `fastpq_bench_manifest.json`-г танилцуулах багцад хадгална уу.
3. **Grafana-г экспортлох.**
   - `FASTPQ Acceleration Overview` самбарт танилцуулах цонхтой тайлбар хийх,
     холбогдох Grafana самбар ID-уудтай холбох.
   - JSON хяналтын самбарыг Grafana API (дээрх тушаал)-аар экспортлох ба оруулах
     `annotations` хэсгийг ашиглан хянагчид үрчлэлтийн муруйг дараахтай тааруулах боломжтой
     үе шаттай танилцуулга.
4. **Ачааны дохиолол.** Ашигласан сэрэмжлүүлгийн дүрмийг (`dashboards/alerts/…`) яг хуулах
   багц руу оруулах замаар. Хэрэв Prometheus дүрмийг хүчингүй болгосон бол оруулна уу
   дарах ялгаа.
5. **Prometheus/OTEL хусах.** Тус бүрээс `fastpq_execution_mode_total{device_class="<matrix>"}`-г аваарай.
   хост (тайзны өмнө болон дараа) дээр нь OTEL тоолуур
   `fastpq.execution_mode_resolutions_total` ба хосолсон
   `telemetry::fastpq.execution_mode` бүртгэлийн оруулгууд. Эдгээр олдворууд үүнийг баталж байна
   GPU-г нэвтрүүлэх нь тогтвортой бөгөөд CPU-ийн албадан буцаалт нь телеметрийг ялгаруулдаг.
6. **Мөр ашиглалтын телеметрийг архивлах.** ExecWitness-ийн кодыг тайлсны дараа
   дэлгэрүүлэхийн тулд үүссэн JSON-г багцын `row_usage/` доор буулгана уу. CI
   Туслагч (`ci/check_fastpq_row_usage.sh`) эдгээр агшин зуурын зургуудыг
   каноник суурь, `ci/check_fastpq_rollout.sh` одоо бүрийг шаарддаг
   TF-5 нотлох баримтыг хавсаргасан байлгахын тулд дор хаяж нэг `row_usage` файлыг илгээнэ үү.
   суллах тасалбар руу.

## Үе шаттай танилцуулах урсгал

Флот бүрт гурван детерминистик үе шатыг ашигла. Зөвхөн гарсны дараа урагшлах
үе шат бүрийн шалгуурыг хангаж, нотлох баримтын багцад баримтжуулсан байна.| Үе шат | Хамрах хүрээ | Гарах шалгуур | Хавсралт |
|-------|-------|---------------|-------------|
| Нисгэгч (P1) | Бүс бүрт 1 удирдлагын хавтгай + 1 мэдээллийн хавтгайн зангилаа | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` 48 цагийн турш ≥90%, Alertmanager-ийн осол тэг, буцах өрөмдлөг. | Хоёр хостоос багц (вандан JSON, туршилтын тайлбартай Grafana экспорт, буцаах бүртгэл). |
| Налуу зам (P2) | Баталгаажуулагчдын ≥50% ба кластер бүрт дор хаяж нэг архивын эгнээ | GPU-ийн гүйцэтгэл 5 өдрийн турш үргэлжилсэн, 1-ээс илүүгүй удаашрал >10минут, Prometheus тоолуурууд нь 60-аад секундын дотор сэргэлтийн дохио өгдөг. | Налуу замын тайлбар, Prometheus хусах ялгаа, Alertmanager дэлгэцийн агшин/логийг харуулсан шинэчлэгдсэн Grafana экспорт. |
| Өгөгдмөл (P3) | Үлдсэн зангилаа; `iroha_config` | дээр FASTPQ өгөгдмөл гэж тэмдэглэгдсэн Гарын үсэг зурсан вандан манифест + Grafana эцсийн үрчлэлтийн муруйг харуулсан экспорт, мөн тохиргооны сэлгэлтийг харуулсан баримтжуулсан буцаах дасгал. | Эцсийн манифест, Grafana JSON, буцаах бүртгэл, тохиргооны өөрчлөлтийг шалгах тасалбарын лавлагаа. |

Сурталчилгааны алхам бүрийг танилцуулах тасалбарт баримтжуулж, шууд холбоно уу
`grafana_fastpq_acceleration.json` аннотаци нь хянагчдыг харьцуулах боломжтой
нотлох баримтын хамт цаг хугацаа.

## Буцах өрөм

Дамжуулах үе шат бүр нь буцаах бэлтгэлийг агуулсан байх ёстой:

1. Кластер бүрээс нэг зангилаа сонгож, одоогийн хэмжүүрүүдийг бичнэ үү:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. Тохиргооны товчлуурыг ашиглан CPU горимыг 10 минутын турш хүчээр дарна уу
   (`zk.fastpq.execution_mode = "cpu"`) эсвэл орчны хүчингүй болгох:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. Бууралтын бүртгэлийг баталгаажуулна уу
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) ба хусах
   Prometheus төгсгөлийн цэгийг дахин ашиглан тоолуурын өсөлтийг харуулна.
4. GPU горимыг сэргээж, `telemetry::fastpq.execution_mode` одоо мэдээлж байгаа эсэхийг шалгана уу
   `resolved="metal"` (эсвэл Металл бус эгнээний хувьд `resolved="cuda"/"opencl"`),
   Prometheus хусах нь CPU болон GPU дээжийг хоёуланг нь агуулж байгааг баталгаажуулна уу.
   `fastpq_execution_mode_total{backend=…}`, мөн өнгөрсөн хугацааг бүртгэнэ үү
   илрүүлэх/цэвэрлэх.
5. Бүрхүүлийн транскрипт, хэмжүүр, операторын баталгаажуулалтыг дараах байдлаар хадгална
   `rollback_drill.log` болон `metrics_rollback.prom` багцын багцад. Эдгээр
   файлууд нь бүрэн бууруулах + сэргээх мөчлөгийг харуулах ёстой, учир нь
   `ci/check_fastpq_rollout.sh` одоо бүртгэлд GPU байхгүй үед бүтэлгүйтдэг
   сэргээх шугам эсвэл хэмжүүрийн агшин зуурын зураг нь CPU эсвэл GPU тоолуурын аль нэгийг орхигдуулдаг.

Эдгээр бүртгэлүүд нь кластер бүр сайнаар доройтож, SRE багууд гэдгийг нотолж байна
Хэрэв GPU драйверууд эсвэл цөмүүд регресс болвол хэрхэн тодорхой хэмжээгээр ухрахаа мэддэг.

## Холимог горимын нөөц нотолгоо (WP2-E.6)

Хост GPU FFT/LDE хэрэгтэй боловч CPU-ийн Посейдон хэш хийх шаардлагатай үед (Үе шат 7 <900 мс-ээр)
шаардлага), дараах олдворуудыг стандарт буцаах логуудтай хамт багцлана:1. **Тохиргооны ялгаа.** Тохируулсан хост-локал даралтыг шалгана уу (эсвэл хавсаргана уу).
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) явахдаа
   `zk.fastpq.execution_mode` хөндөгдөөгүй. Засварыг нэрлэнэ үү
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **Посейдон тоологч хусах.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   Зураг авалт нь `path="cpu_forced"`-г цоожтой алхамаар нэмэгдэж байгааг харуулах ёстой.
   Тухайн төхөөрөмжийн ангиллын GPU FFT/LDE тоолуур. Буцаж авсны дараа хоёр дахь хусах хэрэгтэй
   GPU горим руу буцах тул тоймчид `path="gpu"` мөрийн анкетыг харах боломжтой.

   Гарсан файлыг `wrap_benchmark.py --poseidon-metrics …` руу дамжуулж, боосон жишиг нь `poseidon_metrics` хэсэгт ижил тоолуурыг бүртгэнэ; Энэ нь Металл болон CUDA програмуудыг ижил ажлын урсгал дээр байлгаж, тусдаа хусах файл нээхгүйгээр нөөц нотлох баримтыг шалгах боломжтой болгодог.
3. **Бүртгэлийн ишлэл.** Үүнийг нотлох `telemetry::fastpq.poseidon` оруулгуудыг хуулна уу.
   шийдүүлэгчийг CPU (`cpu_forced`) руу шилжүүлэв
   `poseidon_fallback.log`, Alertmanager-н цагийн хуваарийг хадгалах боломжтой
   тохиргооны өөрчлөлттэй холбоотой.

CI өнөөдөр дараалал/тэг бөглөх шалгалтыг хэрэгжүүлдэг; холимог горимын хаалга газардсаны дараа,
`ci/check_fastpq_rollout.sh` нь мөн аливаа багцыг агуулсан байхыг шаардах болно
`poseidon_fallback.patch` нь тохирох `metrics_poseidon.prom` агшин зуурын зургийг илгээдэг.
Энэхүү ажлын урсгалыг дагаснаар WP2-E.6 нөөцийн бодлогыг аудит хийх боломжтой, холбоотой байлгадаг
өгөгдмөл ашиглалтын явцад ашигласан нотлох баримт цуглуулагчид.

## Тайлан ба автоматжуулалт

- `artifacts/fastpq_rollouts/<stamp>/` лавлахыг бүхэлд нь хавсаргана
  тасалбарыг гаргаж, танилцуулга хаагдсаны дараа `status.md`-ээс лавлана уу.
- `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` ажиллуулна уу (дамжуулан
  `promtool`) нь CI доторх дохионы багцыг танилцуулгатай хамт байлгахын тулд
  эмхэтгэх.
- Багцыг `ci/check_fastpq_rollout.sh` (эсвэл
  `make check-fastpq-rollout`) ба `FASTPQ_ROLLOUT_BUNDLE=<path>`-г давах үед
  нэг удаагийн сурталчилгаанд чиглүүлэхийг хүсч байна. CI нь ижил скриптийг ашиглан дууддаг
  `.github/workflows/fastpq-rollout.yml`, тиймээс алга болсон олдворууд нь a
  суллах тасалбар хааж болно. Хувилбарын шугам нь баталгаажуулсан багцуудыг архивлах боломжтой
  гарын үсэг зурсан манифестуудын хажуугаар
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` хүртэл
  `scripts/run_release_pipeline.py`; туслагч дахин давтав
  `ci/check_fastpq_rollout.sh` (`--skip-fastpq-rollout-check` тохируулаагүй бол) болон
  лавлах модыг `artifacts/releases/<version>/fastpq_rollouts/…` руу хуулна.
  Энэ хаалганы нэг хэсэг болгон скрипт нь Stage7-ийн дарааллын гүн болон тэг дүүргэлтийг хэрэгжүүлдэг.
  `benchmarks.metal_dispatch_queue` болон уншсанаар төсөв
  `metal` вандан JSON бүрээс `benchmarks.zero_fill_hotspots`.

Энэхүү тоглоомын номыг дагаснаар бид детерминист үрчлэлтийг харуулж чадна
танилцуулга бүрт нэг нотлох баримтыг багцалж, буцаах дасгалуудыг аудиттай байлгаарай
гарын үсэг зурсан жишиг илэрнэ.