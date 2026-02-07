---
lang: am
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

# FASTPQ ልቀት ጨዋታ መጽሐፍ (ደረጃ 7-3)

ይህ የመጫወቻ መጽሐፍ የደረጃ 7-3 የመንገድ ካርታ መስፈርትን ተግባራዊ ያደርጋል፡ እያንዳንዱን መርከቦች ማሻሻል
የ FASTPQ ጂፒዩ ማስፈጸሚያን የሚደግም የቤንችማርክ መግለጫ ማያያዝ አለበት፣
የተጣመሩ Grafana ማስረጃዎች፣ እና በሰነድ የተደገፈ የመመለሻ መሰርሰሪያ። ያሟላል።
`docs/source/fastpq_plan.md` (ዒላማዎች/ሥነ ሕንፃ) እና
`docs/source/fastpq_migration_guide.md` (የመስቀለኛ ደረጃ ማሻሻያ ደረጃዎች) በማተኮር
በኦፕሬተር ፊት ለፊት ያለው የታቀዱ የፍተሻ ዝርዝር ላይ።

## ወሰን እና ሚናዎች

- ** የምህንድስና / SRE መልቀቅ፡** የራሱ ቤንችማርክ ቀረጻ፣ አንጸባራቂ ፊርማ እና
  ዳሽቦርድ ከታቀደው መጽደቅ በፊት ወደ ውጭ ይላካል።
- ** Ops Guild:** የታቀዱ ልቀቶችን ያካሂዳል፣ የድጋሚ ልምምዶችን ይመዘግባል እና ያከማቻል
  በ`artifacts/fastpq_rollouts/<timestamp>/` ስር ያለው የቅርስ ቅርቅብ።
- ** አስተዳደር / ተገዢነት፡** ማስረጃዎች ከእያንዳንዱ ለውጥ ጋር እንደሚሄዱ ያረጋግጣል
  ጥያቄ የ FASTPQ ነባሪ ለአንድ መርከቦች ከመቀያየሩ በፊት።

## የማስረጃ ጥቅል መስፈርቶች

እያንዳንዱ የታቀደ ልቀት የሚከተሉትን ቅርሶች መያዝ አለበት። ሁሉንም ፋይሎች ያያይዙ
ወደ ትኬቱ የመልቀቂያ/የማሻሻያ እና ጥቅሉን ያቆዩት።
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| Artefact | ዓላማ | እንዴት ማምረት |
-------------|
| `fastpq_bench_manifest.json` | ቀኖናዊው 20000-ረድፍ የስራ ጫና በ`<1 s` LDE ጣሪያ ስር እንደሚቆይ እና ለእያንዳንዱ የታሸገ ቤንችማርክ ሃሽ መዝግቧል።| ሜታል/CUDAን ያንሱ፣ ያሽጉዋቸው፣ ከዚያ ያሂዱ፡`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \`32X32X03
| የታሸጉ መለኪያዎች (`fastpq_metal_bench_*.json`፣ `fastpq_cuda_bench_*.json`) | የአስተናጋጅ ሜታዳታ፣ የረድፍ አጠቃቀም ማስረጃዎች፣ ዜሮ ሙላ ቦታዎች፣ የፖሲዶን ማይክሮ ቤንች ማጠቃለያዎች እና በዳሽቦርዶች/ማንቂያዎች ጥቅም ላይ የሚውሉ የከርነል ስታቲስቲክሶችን ይያዙ።| `fastpq_metal_bench` / `fastpq_cuda_bench` ን ያሂዱ፣ በመቀጠል ጥሬውን JSON ጠቅልለው፡`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`Prometheus `--row-usage` እና `--poseidon-metrics` በሚመለከታቸው የምስክር/የጭረት ፋይሎች)። ረዳቱ የተጣሩትን `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` ናሙናዎችን ስለሚጨምር WP2-E.6 ማስረጃ በብረት እና በCUDA ላይ አንድ አይነት ነው። ራሱን የቻለ የፖሲዶን ማይክሮ ቤንች ማጠቃለያ ሲፈልጉ `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` ይጠቀሙ (የተጠቀለሉ ወይም ጥሬ ግብዓቶች ይደገፋሉ)። |
|  |  | **የደረጃ7 መለያ መስፈርት፡** `wrap_benchmark.py` አሁን አልተሳካም የ `metadata.labels` ክፍል ሁለቱንም `device_class` እና `gpu_kind` ካልያዘ በስተቀር። አውቶማቲክ ማወቂያ እነሱን መገመት በማይችልበት ጊዜ (ለምሳሌ፣ በተነጣጠለ CI መስቀለኛ መንገድ ላይ ሲታሸጉ) እንደ `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete` ያሉ ግልጽ መሻሮችን ይለፉ። |
|  |  | ** የፍጥነት ቴሌሜትሪ፡** መጠቅለያው እንዲሁ `cargo xtask acceleration-state --format json`ን በነባሪነት ይይዛል፣ `<bundle>.accel.json` እና `<bundle>.accel.prom` በመፃፍ ከተጠቀለለው ቤንችማርክ (በ `--accel-*` ባንዲራዎች ወይም Prometheus)። የቀረጻው ማትሪክስ `acceleration_matrix.{json,md}` ለፍሊት ዳሽቦርዶች ለመገንባት እነዚህን ፋይሎች ይጠቀማል። |
| Grafana ወደ ውጭ መላክ | የጉዲፈቻ ቴሌሜትሪ እና የታቀዱ መስኮቱን የማስጠንቀቂያ ማብራሪያዎችን ያረጋግጣል።| `fastpq-acceleration` ዳሽቦርዱን ወደ ውጭ ይላኩ፡`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`የሰሌዳውን ወደ ውጭ በመላክ ከመጀመሩ በፊት አብራራ። የመልቀቂያ ቧንቧው ይህንን በራስ-ሰር በ `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (በ `GRAFANA_TOKEN` በኩል የቀረበ) ማድረግ ይችላል። |
| ማንቂያ ቅጽበተ | ልቀቱን የሚጠብቀው የማንቂያ ደንቦችን ይይዛል።| ገምጋሚዎች `dashboards/alerts/fastpq_acceleration_rules.yml` (እና የ `tests/` እቃውን) ወደ ቅርቅቡ ይቅዱ። |
| Rollback መሰርሰሪያ መዝገብ | ኦፕሬተሮች የግዳጅ የሲፒዩ ውድቀትን እና የቴሌሜትሪ እውቅናዎችን መለማመዳቸውን ያሳያል።| የአሰራር ሂደቱን በ [Rollback Drills](#rollback-drills) እና የማከማቻ ኮንሶል ምዝግብ ማስታወሻዎችን (`rollback_drill.log`) እና የተገኘውን Prometheus scrape (`metrics_rollback.prom`) ይጠቀሙ። || `row_usage/fastpq_row_usage_<date>.json` | TF-5 በCI እና ዳሽቦርድ ውስጥ የሚከታተለውን የኤክሰክዊትነስ FASTPQ ረድፍ ድልድል ይመዘግባል።| አዲስ ምስክርን ከTorii ያውርዱ፣ በ`iroha_cli audit witness --decode exec.witness` በኩል ይፍቱት (በአማራጭ `--fastpq-parameter fastpq-lane-balanced` ጨምር የሚጠበቀው መለኪያ ስብስብ፣ FASTPQ ስብስቦች በነባሪ ይለቃሉ) እና `row_usage` JSON ወደ `row_usage` JSON ገምጋሚዎች ከታቀደው ትኬቱ ​​ጋር እንዲያዛምዷቸው የፋይል ስሞችን በጊዜ ማህተም ያቆዩ እና `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (ወይም `make check-fastpq-rollout`) ያሂዱ ስለዚህ የደረጃ7-3 በር እያንዳንዱ ቡድን የመራጩን ብዛት እና `transfer_ratio = transfer_rows / total_rows` የማይለዋወጥ ማስረጃውን ከማያያዝዎ በፊት ያረጋግጣል። |

> ** ጠቃሚ ምክር:** `artifacts/fastpq_rollouts/README.md` የሚመርጠውን ስያሜ ሰነዱ
> እቅድ (`<stamp>/<fleet>/<lane>`) እና አስፈላጊው የማስረጃ ሰነዶች። የ
> የ`<stamp>` ማህደር ቅርሶች መደርደር የሚችሉ ሆነው እንዲቆዩ `YYYYMMDDThhmmZ` ኮድ ማድረግ አለበት
> ቲኬቶችን ሳያማክሩ.

## ማስረጃ ማመንጨት ዝርዝር1. ** የጂፒዩ መለኪያዎችን ያንሱ።**
   - ቀኖናዊውን የሥራ ጫና ያሂዱ (20000 ምክንያታዊ ረድፎች ፣ 32768 የታሸጉ ረድፎች) በ
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - ውጤቱን በ`scripts/fastpq/wrap_benchmark.py` ተጠቅልሎ `--row-usage <decoded witness>` በመጠቀም ጥቅሉ የመግብሩን ማስረጃ ከጂፒዩ ቴሌሜትሪ ጋር ይይዛል። `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` ይለፉ ስለዚህ መጠቅለያው ከዒላማው በላይ ካለፈ ወይም የፖሲዶን ወረፋ/መገለጫ ቴሌሜትሪ ከጠፋ እና የተነጠለውን ፊርማ ለማመንጨት በፍጥነት ይከሽፋል።
   - መግለጫው ሁለቱንም የጂፒዩ ቤተሰቦች እንዲይዝ በCUDA አስተናጋጅ ላይ ይድገሙት።
   - ** አታድርግ *** `benchmarks.metal_dispatch_queue` ወይም
     `benchmarks.zero_fill_hotspots` ከተጠቀለለው JSON ያግዳል። CI በር
     (`ci/check_fastpq_rollout.sh`) አሁን እነዚያን መስኮች አንብቦ ሲሰለፍ አይሳካም
     የጭንቅላት ክፍል ከአንድ ማስገቢያ በታች ይወድቃል ወይም ማንኛውም የLDE መገናኛ ነጥብ `mean_ms > ሪፖርት ሲያደርግ
     0.40ms`፣ የStage7 ቴሌሜትሪ ጥበቃን በራስ ሰር በማስፈጸም ላይ።
2. ** አንጸባራቂውን ይፍጠሩ።** `cargo xtask fastpq-bench-manifest …` ይጠቀሙ
   በሰንጠረዡ ውስጥ ይታያል. በታቀደው ጥቅል ውስጥ `fastpq_bench_manifest.json` ያከማቹ።
3. ** Grafana ወደ ውጪ ላክ።**
   - የ `FASTPQ Acceleration Overview` ሰሌዳን በታቀደ መስኮቱ ያብራሩ ፣
     ከሚመለከታቸው Grafana የፓነል መታወቂያዎች ጋር በማገናኘት ላይ።
   - ዳሽቦርዱን JSON በGrafana ኤፒአይ (ከላይ ያለው ትዕዛዝ) ወደ ውጭ ይላኩ እና ያካትቱ
     የ `annotations` ክፍል ስለዚህ ገምጋሚዎች የማደጎ ኩርባዎችን ከ
     የታቀደ ልቀት.
4. ** ቅጽበታዊ ማንቂያዎች።** ያገለገሉትን ትክክለኛ የማንቂያ ደንቦች (`dashboards/alerts/…`) ይቅዱ።
   ወደ ጥቅል ውስጥ በመልቀቅ. Prometheus ደንቦች ከተሻሩ ያካትቱ
   የመሻር ልዩነት.
5. **Prometheus/OTEL መቧጨር።** `fastpq_execution_mode_total{device_class="<matrix>"}` ከእያንዳንዱ ያንሱ
   አስተናጋጅ (ከመድረክ በፊት እና በኋላ) እና የኦቲኤል ቆጣሪ
   `fastpq.execution_mode_resolutions_total` እና የተጣመሩ
   `telemetry::fastpq.execution_mode` ምዝግብ ማስታወሻዎች. እነዚህ ቅርሶች ይህን ያረጋግጣሉ
   የጂፒዩ ጉዲፈቻ የተረጋጋ ነው እና ያስገደደው የሲፒዩ ውድቀት አሁንም ቴሌሜትሪ ያመነጫል።
6. ** የረድፍ አጠቃቀም ቴሌሜትሪ በማህደር ውስጥ።** ExecWitness ሩጫ ለ
   መልቀቅ፣ የተገኘውን JSON በ `row_usage/` በጥቅሉ ውስጥ ጣል። ሲ.አይ
   አጋዥ (`ci/check_fastpq_row_usage.sh`) እነዚህን ቅጽበታዊ ገጽ እይታዎች ከ
   ቀኖናዊ መነሻ መስመሮች፣ እና `ci/check_fastpq_rollout.sh` አሁን እያንዳንዱን ይፈልጋል
   የTF-5 ማስረጃን ተያይዞ ለማቆየት ቢያንስ አንድ `row_usage` ፋይል ለመላክ ጥቅል
   ወደ መልቀቂያ ትኬት.

## የታቀደ ልቀት ፍሰት

ለእያንዳንዱ መርከቦች ሶስት የመወሰን ደረጃዎችን ይጠቀሙ። ከመውጣቱ በኋላ ብቻ ይቅደም
በእያንዳንዱ ደረጃ ውስጥ ያሉ መስፈርቶች ረክተዋል እና በማስረጃ ጥቅል ውስጥ ተጽፈዋል።| ደረጃ | ወሰን | መውጫ መስፈርት | ማያያዣዎች |
|---
| አብራሪ (P1) | 1 መቆጣጠሪያ-አይሮፕላን + 1 የውሂብ-አውሮፕላን መስቀለኛ መንገድ በክልል | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90% ለ 48h፣ ዜሮ የማስጠንቀቂያ ማናጀር ክስተቶች እና የማለፊያ ጥቅልል ​​መሰርሰሪያ። | ከሁለቱም አስተናጋጆች ቅርቅብ (ቤንች JSONs፣ Grafana ወደ ውጪ መላክ ከአብራሪ ማብራሪያ፣ ከጥቅልል ምዝግብ ማስታወሻዎች)። |
| ራምፕ (P2) | ≥50% አረጋጋጮች እና ቢያንስ አንድ ማህደር መስመር በክላስተር | የጂፒዩ አፈፃፀሙ ለ 5 ቀናት የዘለቀ፣ ከ1 የመውረድ ፍጥነት >10 ደቂቃ ያልበለጠ፣ እና Prometheus ቆጣሪዎች በ60ዎቹ ውስጥ የውድቀት ማንቂያዎችን ያረጋግጣሉ። | የተሻሻለ Grafana ወደ ውጭ መላክ የራምፕ ማብራሪያ፣ Prometheus scrape diffs፣ Alertmanager screenshot/log. |
| ነባሪ (P3) | የቀሩ አንጓዎች; FASTPQ በ `iroha_config` ውስጥ ነባሪ ምልክት ተደርጎበታል | የተፈረመበት የቤንች ማኒፌክት + Grafana ወደ ውጪ መላክ የመጨረሻውን የጉዲፈቻ ጥምዝ እና የውቅረት መቀያየርን የሚያሳይ የድጋሚ ልምላሜ ሰነድ ነው። | የመጨረሻ አንጸባራቂ፣ Grafana JSON፣ የድጋሚ መዝገብ፣ የቲኬት ማጣቀሻ ለውጥ ግምገማ። |

በታቀደው ትኬት ውስጥ ያለውን እያንዳንዱን የማስተዋወቂያ ደረጃ ይመዝግቡ እና በቀጥታ ወደ
`grafana_fastpq_acceleration.json` ማብራሪያዎች ስለዚህ ገምጋሚዎች ማዛመድ ይችላሉ።
የጊዜ መስመር ከማስረጃ ጋር።

## የጥቅልል ልምምዶች

እያንዳንዱ የታቀፈ ደረጃ የድጋሚ ልምምድ ማካተት አለበት፡-

1. በክላስተር አንድ መስቀለኛ መንገድ ይምረጡ እና የአሁኑን መለኪያዎች ይመዝግቡ፡
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. ሁለቱንም የማዋቀር ቁልፍ በመጠቀም የሲፒዩ ሁነታን ለ10 ደቂቃ አስገድድ
   (`zk.fastpq.execution_mode = "cpu"`) ወይም አካባቢው ይሽራል፡-
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. የማውረድ ምዝግብ ማስታወሻውን ያረጋግጡ
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) እና መቧጨር
   የቆጣሪ ጭማሪዎችን ለማሳየት የ Prometheus የመጨረሻ ነጥብ እንደገና።
4. የጂፒዩ ሁነታን ወደነበረበት መልስ፣ `telemetry::fastpq.execution_mode` አሁን እንደዘገበው ያረጋግጡ
   `resolved="metal"` (ወይም `resolved="cuda"/"opencl"` ለብረት ያልሆኑ መስመሮች)
   የPrometheus መቧጨር ሁለቱንም የሲፒዩ እና የጂፒዩ ናሙናዎችን እንደያዘ ያረጋግጡ
   `fastpq_execution_mode_total{backend=…}`፣ እና ያለፈውን ጊዜ ይመዝገቡ
   ማወቂያ/ማጽዳት.
5. የሼል ግልባጮችን፣ መለኪያዎችን እና የኦፕሬተርን እውቅና እንደ
   በታቀደው ጥቅል ውስጥ `rollback_drill.log` እና `metrics_rollback.prom`። እነዚህ
   ፋይሎች ሙሉውን የውድቀት + ወደነበረበት መመለስ ዑደት ማሳየት አለባቸው ምክንያቱም
   `ci/check_fastpq_rollout.sh` ምዝግብ ማስታወሻው ጂፒዩ በሌለው ቁጥር አሁን አይሳካም።
   የመልሶ ማግኛ መስመር ወይም የሜትሪዎቹ ቅጽበታዊ ገጽ እይታ ሲፒዩ ወይም ጂፒዩ ቆጣሪዎችን ይተዋል ።

እነዚህ ምዝግብ ማስታወሻዎች እያንዳንዱ ዘለላ በሚያምር ሁኔታ እና SRE ቡድኖችን እንደሚያዋርዱ ያረጋግጣሉ
የጂፒዩ ሾፌሮች ወይም ከርነሎች ወደ ኋላ ከተመለሱ በቆራጥነት ወደ ኋላ እንዴት እንደሚመለሱ ይወቁ።

## ቅይጥ ሁነታ የመመለሻ ማስረጃ (WP2-E.6)

አንድ አስተናጋጅ ጂፒዩ FFT/LDE ነገር ግን ሲፒዩ ፖሲዶን ሃሺንግ በሚያስፈልገው ጊዜ (በደረጃ 7 <900ms)
መስፈርት)፣ የሚከተሉትን ቅርሶች ከመደበኛው የመመለሻ ምዝግብ ማስታወሻዎች ጋር ሰብስብ፡-1. **ልዩነትን ያዋቅሩ።** የአስተናጋጅ-አካባቢያዊ መሻርን ያረጋግጡ (ወይም አያያይዙ)
   እየወጣ ሳለ `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`)
   `zk.fastpq.execution_mode` ያልተነካ። ማጣበቂያውን ይሰይሙ
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **የፖሲዶን ቆጣሪ መቧጨር።**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   ቀረጻው የ`path="cpu_forced"` ጭማሪን በመቆለፊያ ደረጃ ማሳየት አለበት
   ለዚያ መሣሪያ-ክፍል ጂፒዩ FFT/LDE ቆጣሪ። ከተመለስክ በኋላ ሁለተኛ መፋቅ ውሰድ
   ገምጋሚዎች የGrafana ረድፍ ከቆመበት ቀጥል እንዲያዩ ወደ ጂፒዩ ሁነታ ተመለስ።

   የተገኘውን ፋይል ወደ `wrap_benchmark.py --poseidon-metrics …` ያስተላልፉ ስለዚህ የተጠቀለለው ቤንችማርክ በ `poseidon_metrics` ክፍል ውስጥ ያሉትን ተመሳሳይ ቆጣሪዎች ይመዘግባል; ይህ የብረታ ብረት እና የ CUDA ልቀቶችን በተመሳሳይ የስራ ሂደት ላይ ያቆያል እና የተለየ የመቧጠጫ ፋይሎችን ሳይከፍቱ የመመለሻ ማስረጃዎችን ኦዲት ያደርጋል።
3. ** የምዝግብ ማስታወሻ ጽሁፍ።** የ `telemetry::fastpq.poseidon` ግቤቶችን ያረጋግጣሉ።
   መፍታት ወደ ሲፒዩ (`cpu_forced`) ተገለበጠ
   `poseidon_fallback.log`፣ የአለርትማናጀር የጊዜ መስመሮች እንዲችሉ የጊዜ ማህተሞችን በመጠበቅ ላይ።
   ከውቅረት ለውጥ ጋር የተዛመደ።

CI ዛሬ ወረፋውን / ዜሮ መሙላት ቼኮችን ያስፈጽማል; አንድ ጊዜ የድብልቅ ሁነታ በር መሬት,
`ci/check_fastpq_rollout.sh` ማንኛውንም ጥቅል የያዘ መሆኑን አጥብቆ ይጠይቃል
`poseidon_fallback.patch` ተዛማጅ የሆነውን `metrics_poseidon.prom` ቅጽበተ ፎቶን ይልካል።
ይህንን የስራ ሂደት ተከትሎ የ WP2-E.6 ውድቀት ፖሊሲ ኦዲት ተደርጎ እንዲታይ ያደርገዋል
በነባሪ መልቀቅ ወቅት ጥቅም ላይ የዋሉ ተመሳሳይ ማስረጃ ሰብሳቢዎች።

## ሪፖርት ማድረግ እና አውቶማቲክ

- ሙሉውን የ `artifacts/fastpq_rollouts/<stamp>/` ማውጫን ከ
  ትኬቱን መልቀቅ እና ከ`status.md` መልቀቅ አንዴ ሲዘጋ ያጣቅሰው።
- `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` አሂድ (በ
  `promtool`) በCI ውስጥ የማስጠንቀቂያ ቅርቅቦች ከታቀዱ ጋር መያዛቸውን ለማረጋገጥ
  ማጠናቀር.
- ጥቅሉን በ`ci/check_fastpq_rollout.sh` (ወይም
  `make check-fastpq-rollout`) እና `FASTPQ_ROLLOUT_BUNDLE=<path>` ይለፉ
  ነጠላ ልቀትን ማነጣጠር ይፈልጋሉ። CI ተመሳሳይ ስክሪፕት በ በኩል ይጠራል
  `.github/workflows/fastpq-rollout.yml`፣ስለዚህ የጎደሉ ቅርሶች ከሀ በፊት በፍጥነት ይወድቃሉ
  የመልቀቅ ትኬት ሊዘጋ ይችላል። የሚለቀቀው ቧንቧ መስመር የተረጋገጡ ጥቅሎችን በማህደር ማስቀመጥ ይችላል።
  በማለፍ ከተፈረሙት መግለጫዎች ጎን ለጎን
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` ወደ
  `scripts/run_release_pipeline.py`; ረዳቱ እንደገና ይሮጣል
  `ci/check_fastpq_rollout.sh` (`--skip-fastpq-rollout-check` ካልተቀናበረ በስተቀር) እና
  የማውጫውን ዛፍ ወደ `artifacts/releases/<version>/fastpq_rollouts/…` ይገለበጣል.
  እንደ የዚህ በር አካል ስክሪፕቱ የStage7 ወረፋ ጥልቀት እና ዜሮ መሙላትን ያስገድዳል።
  በጀቶች `benchmarks.metal_dispatch_queue` እና
  `benchmarks.zero_fill_hotspots` ከእያንዳንዱ `metal` አግዳሚ ወንበር JSON።

ይህንን የጨዋታ መጽሐፍ በመከተል ቆራጥ ጉዲፈቻን ማሳየት እንችላለን፣ ሀ
ነጠላ የማስረጃ ጥቅል በአንድ ልቀት፣ እና የድጋሚ ልምምዶችን ከጎን ኦዲት ያድርጉ
የተፈረመው ቤንችማርክ ይገለጣል.