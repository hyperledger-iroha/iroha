---
lang: ka
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

# FASTPQ Rollout Playbook (სტადია7-3)

ეს სახელმძღვანელო ახორციელებს Stage7-3 საგზაო რუკის მოთხოვნას: ფლოტის ყოველი განახლება
რომელიც საშუალებას აძლევს FASTPQ GPU-ს შესრულებას, უნდა დაურთოს რეპროდუცირებადი საორიენტაციო მანიფესტი,
დაწყვილებული Grafana მტკიცებულება და დოკუმენტირებული უკან დაბრუნება. ის ავსებს
`docs/source/fastpq_plan.md` (მიზნები/არქიტექტურა) და
`docs/source/fastpq_migration_guide.md` (კვანძის დონის განახლების ნაბიჯები) ფოკუსირებით
ოპერატორის წინაშე განლაგების საკონტროლო სიაში.

## სფერო და როლები

- ** გამოშვების ინჟინერია / SRE: ** საკუთარი საორიენტაციო აღბეჭდვა, მანიფესტის ხელმოწერა და
  დაფის ექსპორტი გაშვების დამტკიცებამდე.
- **Ops Guild:** აწარმოებს დადგმულ გამოშვებებს, ჩაწერს რეპეტიციებს და ინახავს
  არტეფაქტის ნაკრები `artifacts/fastpq_rollouts/<timestamp>/`-ის ქვეშ.
- **მმართველობა / შესაბამისობა: ** ამოწმებს, რომ მტკიცებულებები თან ახლავს ყველა ცვლილებას
  მოთხოვნა FASTPQ ნაგულისხმევი ფლოტისთვის გადართულია.

## მტკიცებულებათა ნაკრების მოთხოვნები

ყოველი წარდგენის წარდგენა უნდა შეიცავდეს შემდეგ არტეფაქტებს. მიამაგრეთ ყველა ფაილი
გამოშვების/განახლების ბილეთამდე და შეინახეთ პაკეტი
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| არტეფაქტი | დანიშნულება | როგორ ვაწარმოოთ |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` | ადასტურებს, რომ კანონიკური 20000 მწკრივი დატვირთვა რჩება `<1 s` LDE ჭერის ქვეშ და აღრიცხავს ჰეშებს ყველა შეფუთული საორიენტაციო ნიშნისთვის.| გადაიღეთ Metal/CUDA გაშვებები, შეფუთეთ ისინი, შემდეგ გაუშვით:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \`Prometheus
| შეფუთული ბენჩმარკები (`fastpq_metal_bench_*.json`, `fastpq_cuda_bench_*.json`) | აღბეჭდეთ ჰოსტის მეტამონაცემები, მწკრივის გამოყენების მტკიცებულება, ნულოვანი შევსების ცხელი წერტილები, Poseidon microbench შეჯამებები და ბირთვის სტატისტიკა, რომელიც გამოიყენება საინფორმაციო დაფებით/გაფრთხილებებით.| გაუშვით `fastpq_metal_bench` / `fastpq_cuda_bench`, შემდეგ შეფუთეთ დაუმუშავებელი JSON:`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`I100Re4000-ისთვის (წერტილი `--row-usage` და `--poseidon-metrics` შესაბამის მოწმეზე/შეასწორეთ ფაილები). დამხმარე ათავსებს გაფილტრულ `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` ნიმუშებს, ასე რომ WP2-E.6 მტკიცებულება იდენტურია Metal-სა და CUDA-ში. გამოიყენეთ `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`, როდესაც გჭირდებათ Poseidon-ის მიკროსკოპის დამოუკიდებელი რეზიუმე (შეფუთული ან ნედლი შეყვანის მხარდაჭერა). |
|  |  | **Stage7 ეტიკეტის მოთხოვნა:** `wrap_benchmark.py` ახლა ვერ ხერხდება, თუ შედეგად `metadata.labels` სექცია შეიცავს `device_class` და `gpu_kind`. როდესაც ავტომატური ამოცნობა მათ ვერ გამოიტანს (მაგალითად, მოწყვეტილ CI კვანძზე შეფუთვისას), გაიარეთ აშკარა უგულებელყოფა, როგორიცაა `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`. |
|  |  | **აჩქარების ტელემეტრია:** შეფუთვა ასევე იჭერს `cargo xtask acceleration-state --format json` ნაგულისხმევად, წერს `<bundle>.accel.json` და `<bundle>.accel.prom` შეფუთული საორიენტაციო ნიშნის გვერდით (გადალახავს `--accel-*` დროშებით `--accel-*`500X ან I01). გადაღების მატრიცა იყენებს ამ ფაილებს ფლოტის დაფებისთვის `acceleration_matrix.{json,md}`-ის შესაქმნელად. |
| Grafana ექსპორტი | ადასტურებს მიღების ტელემეტრიას და გაფრთხილების ანოტაციებს გაშვების ფანჯრისთვის.| ექსპორტი `fastpq-acceleration` საინფორმაციო დაფა:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`გაანაწილეთ დაფა ექსპორტის დაწყებამდე/ზედა ჯერ. გამოშვების მილსადენს შეუძლია ამის გაკეთება ავტომატურად `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>`-ის მეშვეობით (ჟეტონი მოწოდებულია `GRAFANA_TOKEN`-ის მეშვეობით). |
| Alert Snapshot | იჭერს გაფრთხილების წესებს, რომლებიც იცავდნენ გაშვებას.| დააკოპირეთ `dashboards/alerts/fastpq_acceleration_rules.yml` (და `tests/` მოწყობილობა) პაკეტში, რათა მიმომხილველებმა შეძლონ ხელახლა გაშვება `promtool test rules …`. |
| გადაბრუნების საბურღი ჟურნალი | ადასტურებს, რომ ოპერატორებმა გაიმეორეს CPU-ის იძულებითი გამობრუნების და ტელემეტრიის აღიარება.| გამოიყენეთ პროცედურა [Rollback Drills]-ში (#rollback-drills) და შეინახეთ კონსოლის ჟურნალები (`rollback_drill.log`) პლუს მიღებული Prometheus ნაკაწრი (`metrics_rollback.prom`). || `row_usage/fastpq_row_usage_<date>.json` | ჩაწერს ExecWitness FASTPQ მწკრივის განაწილებას, რომელსაც TF-5 აკონტროლებს CI-ში და დაფებში.| ჩამოტვირთეთ ახალი მოწმობა Torii-დან, გაშიფრეთ იგი `iroha_cli audit witness --decode exec.witness`-ის საშუალებით (სურვილისამებრ დაამატეთ `--fastpq-parameter fastpq-lane-balanced` მოსალოდნელი პარამეტრის ნაკრების დასამტკიცებლად; FASTPQ სერიები ასხივებენ ნაგულისხმევად) და დააკოპირეთ Prometheus-ში. შეინახეთ ფაილების სახელები დროის შტამპით, რათა მიმომხილველებმა შეძლონ მათი კორელაცია გაშვების ბილეთთან და გაუშვან `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (ან `make check-fastpq-rollout`), რათა Stage7-3 კარი ამოწმებდეს, რომ ყოველი პარტია აქვეყნებს რეკლამას სელექტორის რაოდენობას და `transfer_ratio = transfer_rows / total_rows`-ს მიმაგრებამდე. |

> **მინიშნება:** `artifacts/fastpq_rollouts/README.md` აფიქსირებს სასურველ დასახელებას
> სქემა (`<stamp>/<fleet>/<lane>`) და საჭირო მტკიცებულებათა ფაილები. The
> `<stamp>` საქაღალდეში უნდა იყოს კოდირებული `YYYYMMDDThhmmZ`, რათა არტეფაქტები დარჩეს დასალაგებლად
> ბილეთების კონსულტაციის გარეშე.

## მტკიცებულებების გენერირების საკონტროლო სია1. **გადაიღეთ GPU საორიენტაციო ნიშნები.**
   - გაუშვით კანონიკური დატვირთვა (20000 ლოგიკური მწკრივი, 32768 შეფუთული მწკრივი) მეშვეობით
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - შეფუთეთ შედეგი `scripts/fastpq/wrap_benchmark.py`-ით `--row-usage <decoded witness>`-ით, რათა პაკეტმა გადაიტანოს გაჯეტის მტკიცებულება GPU ტელემეტრიასთან ერთად. გაიარეთ `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` ისე, რომ შეფუთვა სწრაფად ჩავარდეს, თუ რომელიმე ამაჩქარებელი აჭარბებს მიზანს, ან თუ პოსეიდონის რიგის/პროფილის ტელემეტრია აკლია, და მოაწყვეთ ხელმოწერის გენერირება.
   - გაიმეორეთ CUDA ჰოსტზე, რათა მანიფესტი შეიცავდეს ორივე GPU ოჯახს.
   - ** არ მოაშოროთ `benchmarks.metal_dispatch_queue` ან
     `benchmarks.zero_fill_hotspots` ბლოკავს შეფუთული JSON-დან. CI კარიბჭე
     (`ci/check_fastpq_rollout.sh`) ახლა კითხულობს ამ ველებს და ვერ დგას რიგში
     headroom ეცემა ერთ სლოტზე ქვემოთ ან როდესაც რომელიმე LDE Hotspot იტყობინება `mean_ms >
     0.40ms`, ავტომატურად ახორციელებს Stage7 ტელემეტრიის დაცვას.
2. ** შექმენით manifest.** გამოიყენეთ `cargo xtask fastpq-bench-manifest …` როგორც
   ნაჩვენებია ცხრილში. შეინახეთ `fastpq_bench_manifest.json` გამოშვების პაკეტში.
3. **ექსპორტი Grafana.**
   - შეიტანეთ ანოტირება `FASTPQ Acceleration Overview` დაფაზე გასაშლელი ფანჯრით,
     დაკავშირება შესაბამის Grafana პანელის ID-ებთან.
   - JSON დაფის ექსპორტი Grafana API-ით (ზემოთ ბრძანება) და ჩართეთ
     `annotations` განყოფილება, რათა მიმომხილველებმა შეძლონ შვილად აყვანის მრუდების შედარება
     ეტაპობრივი გაშვება.
4. **Snapshot-ის გაფრთხილებები.** დააკოპირეთ ზუსტი გაფრთხილების წესები (`dashboards/alerts/…`) გამოყენებული
   პაკეტში გადატანის გზით. თუ Prometheus წესები გაუქმდა, ჩართეთ
   გადაფარვის განსხვავება.
5. **Prometheus/OTEL scrape.** აღბეჭდეთ `fastpq_execution_mode_total{device_class="<matrix>"}` თითოეულიდან
   მასპინძელი (სცენის წინ და შემდეგ) პლუს OTEL-ის დახლი
   `fastpq.execution_mode_resolutions_total` და დაწყვილებული
   `telemetry::fastpq.execution_mode` ჟურნალის ჩანაწერები. ეს არტეფაქტები ამას ადასტურებს
   GPU-ს მიღება სტაბილურია და CPU იძულებითი ჩანაცვლება კვლავ ასხივებს ტელემეტრიას.
6. **დაარქივეთ მწკრივის გამოყენების ტელემეტრია.** ExecWitness-ის გაშიფვრის შემდეგ გაუშვით
   გამოაქვეყნეთ, ჩამოაგდეთ მიღებული JSON `row_usage/` ქვეშ პაკეტში. CI
   დამხმარე (`ci/check_fastpq_row_usage.sh`) ადარებს ამ კადრებს
   კანონიკური საბაზისო ხაზები და `ci/check_fastpq_rollout.sh` ახლა მოითხოვს ყველა
   შეფუთეთ მინიმუმ ერთი `row_usage` ფაილის გასაგზავნად TF-5 მტკიცებულების მიმაგრების მიზნით
   გათავისუფლების ბილეთამდე.

## ეტაპობრივი გაშვების ნაკადი

გამოიყენეთ სამი დეტერმინისტული ფაზა თითოეული ფლოტისთვის. წინსვლა მხოლოდ გასვლის შემდეგ
კრიტერიუმები თითოეულ ფაზაში დაკმაყოფილებულია და დოკუმენტირებულია მტკიცებულებათა პაკეტში.| ფაზა | ფარგლები | გასვლის კრიტერიუმები | დანართები |
|-------|-------|--------------|-------------|
| პილოტი (P1) | 1 საკონტროლო სიბრტყე + 1 მონაცემთა სიბრტყის კვანძი თითო რეგიონში | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90% 48 სთ-ისთვის, ნულოვანი Alertmanager ინციდენტები და უკან დაბრუნებული სავარჯიშო. | შეფუთვა ორივე ჰოსტიდან (მჯდარი JSON-ები, Grafana ექსპორტი საპილოტე ანოტაციით, დაბრუნების ჟურნალები). |
| Ramp (P2) | ≥50% ვალიდატორები პლუს მინიმუმ ერთი საარქივო ხაზი თითო კლასტერზე | GPU-ს შესრულება გაგრძელდა 5 დღის განმავლობაში, არაუმეტეს 1 დაქვეითების მწვერვალი > 10 წთ. | განახლებულია Grafana ექსპორტი, რომელიც გვიჩვენებს რამპის ანოტაციას, Prometheus სკრეპის განსხვავებებს, Alertmanager ეკრანის ანაბეჭდს/ ჟურნალს. |
| ნაგულისხმევი (P3) | დარჩენილი კვანძები; FASTPQ მონიშნულია ნაგულისხმევად `iroha_config` | ხელმოწერილი სკამზე მანიფესტი + Grafana ექსპორტი, რომელიც მიუთითებს მიღების საბოლოო მრუდზე, და დოკუმენტირებული საბურღი, რომელიც აჩვენებს კონფიგურაციის გადართვას. | საბოლოო მანიფესტი, Grafana JSON, დაბრუნების ჟურნალი, ბილეთის მითითება კონფიგურაციის ცვლილების მიმოხილვაზე. |

დაარეგისტრირეთ თითოეული სარეკლამო ნაბიჯი ბილეთში და პირდაპირ დაუკავშირდით მას
`grafana_fastpq_acceleration.json` ანოტაციები, რათა რეცენზენტებმა შეძლონ მათი კორელაცია
ვადები მტკიცებულებებით.

## დაბრუნების სავარჯიშოები

გაშვების ყოველი ეტაპი უნდა მოიცავდეს დაბრუნების რეპეტიციას:

1. აირჩიეთ ერთი კვანძი თითო კლასტერზე და ჩაწერეთ მიმდინარე მეტრიკა:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. აიძულეთ CPU რეჟიმი 10 წუთის განმავლობაში კონფიგურაციის ღილაკის გამოყენებით
   (`zk.fastpq.execution_mode = "cpu"`) ან გარემოს უგულებელყოფა:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. დაადასტურეთ შემცირების ჟურნალი
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) და გახეხეთ
   ისევ Prometheus საბოლოო წერტილი მრიცხველის ნამატების საჩვენებლად.
4. აღადგინეთ GPU რეჟიმი, დაადასტურეთ, რომ `telemetry::fastpq.execution_mode` ახლა იუწყება
   `resolved="metal"` (ან `resolved="cuda"/"opencl"` არამეტალური ზოლებისთვის),
   დაადასტურეთ, რომ Prometheus ნაკაწრი შეიცავს როგორც CPU, ასევე GPU ნიმუშებს
   `fastpq_execution_mode_total{backend=…}` და დაარეგისტრირეთ გასული დრო
   გამოვლენა/გასუფთავება.
5. შეინახეთ გარსის ტრანსკრიპტები, მეტრიკა და ოპერატორის დადასტურება როგორც
   `rollback_drill.log` და `metrics_rollback.prom` გამოშვების პაკეტში. ესენი
   ფაილები უნდა ასახავდეს სრული დაქვეითების + აღდგენის ციკლს, რადგან
   `ci/check_fastpq_rollout.sh` ახლა მარცხდება, როდესაც ჟურნალს აკლია GPU
   აღდგენის ხაზი ან მეტრიკის სნეპშოტი გამოტოვებს CPU ან GPU მრიცხველებს.

ეს ჟურნალები ადასტურებს, რომ ყველა კლასტერს შეუძლია მოხდენილი დეგრადაცია და რომ SRE გუნდები
იცოდეთ როგორ დაბრუნდეთ დეტერმინისტულად, თუ GPU დრაივერები ან ბირთვები რეგრესდებიან.

## შერეული რეჟიმის სარეზერვო მტკიცებულება (WP2-E.6)

როდესაც მასპინძელს სჭირდება GPU FFT/LDE, მაგრამ CPU Poseidon ჰეშირება (7 ეტაპის მიხედვით <900 ms
მოთხოვნა), შეაერთეთ შემდეგი არტეფაქტები სტანდარტული უკან დაბრუნების ჟურნალებთან ერთად:1. **Config diff.** შეამოწმეთ (ან მიამაგრეთ) ჰოსტის ლოკალური გადაფარვა, რომელიც დაყენებულია
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) გასვლისას
   `zk.fastpq.execution_mode` ხელუხლებელი. დაასახელეთ პაჩი
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **პოსეიდონის მრიცხველის საფხეკი.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   გადაღება უნდა აჩვენოს `path="cpu_forced"` მატება დაბლოკვის საფეხურზე
   GPU FFT/LDE მრიცხველი ამ მოწყობილობის კლასისთვის. წაიღეთ მეორე ნაკაწრი დაბრუნების შემდეგ
   დაუბრუნდით GPU რეჟიმში, რათა მიმომხილველებმა ნახონ `path="gpu"` რიგის რეზიუმე.

   მიღებული ფაილი გადაიტანეთ `wrap_benchmark.py --poseidon-metrics …`-ზე, რათა შეფუთული ბენჩმარკი ჩაიწეროს იგივე მრიცხველები მის `poseidon_metrics` განყოფილებაში; ეს ინარჩუნებს Metal და CUDA rollouts-ს იდენტურ სამუშაო პროცესზე და აქცევს სარეზერვო მტკიცებულებებს აუდიტორად ცალკეული scrape ფაილების გახსნის გარეშე.
3. ** ჟურნალის ამონაწერი.** დააკოპირეთ `telemetry::fastpq.poseidon` ჩანაწერები, რომლებიც ადასტურებენ
   გადამწყვეტი გადატრიალდა CPU-ში (`cpu_forced`).
   `poseidon_fallback.log`, დროის შტამპების შენახვა, რათა Alertmanager-ის ვადები იყოს
   დაკავშირებულია კონფიგურაციის ცვლილებასთან.

CI ახორციელებს რიგის/ნულოვანი შევსების შემოწმებებს დღეს; როგორც კი შერეული რეჟიმის კარიბჭე დაეშვება,
`ci/check_fastpq_rollout.sh` ასევე დაჟინებით მოითხოვს ნებისმიერი პაკეტის შემცველობას
`poseidon_fallback.patch` აგზავნის შესაბამის `metrics_poseidon.prom` სურათს.
ამ სამუშაო პროცესის შემდეგ WP2-E.6 სარეზერვო პოლიტიკა ინარჩუნებს აუდიტს და მიბმულს
იგივე მტკიცებულებების შემგროვებლები, რომლებიც გამოყენებული იქნა ნაგულისხმევი გაშვების დროს.

## მოხსენება და ავტომატიზაცია

- მიამაგრეთ მთელი `artifacts/fastpq_rollouts/<stamp>/` დირექტორია
  გამოუშვით ბილეთი და მიმართეთ მას `status.md`-დან, როგორც კი გაშვება დაიხურება.
- გაუშვით `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (მეშვეობით
  `promtool`) CI-ში, რათა უზრუნველყოს გაფრთხილების პაკეტები შეფუთული გაშვებით
  შედგენა.
- დაადასტურეთ პაკეტი `ci/check_fastpq_rollout.sh`-ით (ან
  `make check-fastpq-rollout`) და გაიარეთ `FASTPQ_ROLLOUT_BUNDLE=<path>`, როცა
  გსურთ მიზანმიმართული ერთი rollout. CI იწვევს იმავე სკრიპტს მეშვეობით
  `.github/workflows/fastpq-rollout.yml`, ამიტომ დაკარგული არტეფაქტები სწრაფად იშლება ადრე a
  გამოშვების ბილეთი შეიძლება დაიხუროს. გამოშვების მილსადენს შეუძლია დაარქივოს დადასტურებული პაკეტები
  ხელმოწერილის გვერდით მანიფესტებს გავლის გზით
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` to
  `scripts/run_release_pipeline.py`; დამხმარე მეორდება
  `ci/check_fastpq_rollout.sh` (თუ `--skip-fastpq-rollout-check` არ არის დაყენებული) და
  აკოპირებს დირექტორია ხეს `artifacts/releases/<version>/fastpq_rollouts/…`-ში.
  როგორც ამ კარიბჭის ნაწილი, სკრიპტი ახორციელებს Stage7 რიგის სიღრმეს და ნულოვანი შევსებას
  ბიუჯეტი `benchmarks.metal_dispatch_queue` წაკითხვით და
  `benchmarks.zero_fill_hotspots` თითოეული `metal` JSON სკამიდან.

ამ სახელმძღვანელოს მიყოლებით ჩვენ შეგვიძლია ვაჩვენოთ დეტერმინისტული მიღება, მივაწოდოთ ა
ერთჯერადი მტკიცებულების ნაკრები თითო გამოშვებაში და განაგრძეთ უკან დაბრუნება სავარჯიშოების აუდიტი ერთად
ხელმოწერილი ნიშნული ვლინდება.