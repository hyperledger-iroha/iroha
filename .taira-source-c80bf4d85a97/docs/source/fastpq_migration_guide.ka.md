---
lang: ka
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ წარმოების მიგრაციის გზამკვლევი

ეს runbook აღწერს, თუ როგორ უნდა დაადასტუროთ Stage6 წარმოების FASTPQ პროვერი.
ამ მიგრაციის გეგმის ფარგლებში წაიშალა განმსაზღვრელი ჩანაცვლების ადგილი.
ის ავსებს დადგმულ გეგმას `docs/source/fastpq_plan.md`-ში და ვარაუდობს, რომ თქვენ უკვე ადევნებთ თვალყურს
სამუშაო სივრცის სტატუსი `status.md`-ში.

## აუდიტორია და სფერო
- Validator ოპერატორები ამუშავებენ წარმოების პროვერს ინსცენირების ან mainnet გარემოში.
- გაათავისუფლეთ ინჟინრები, რომლებიც ქმნიან ბინარებს ან კონტეინერებს, რომლებიც გაიგზავნება პროდუქციის ფონზე.
- SRE/დაკვირვებადობის გუნდები ახალი ტელემეტრიული სიგნალების გაყვანილობას და სიგნალიზაციას.

ფარგლებს გარეთ: Kotodama კონტრაქტის ავტორიზაცია და IVM ABI ცვლილებები (იხილეთ `docs/source/nexus.md`
შესრულების მოდელი).

## მხატვრული მატრიცა
| ბილიკი | ტვირთის მახასიათებლები | შედეგი | როდის გამოვიყენოთ |
| ---- | ----------------------- | ------ | ----------- |
| წარმოების პროვერი (ნაგულისხმევი) | _არცერთი_ | Stage6 FASTPQ backend FFT/LDE დამგეგმავებით და DEEP-FRI მილსადენით.【crates/fastpq_prover/src/backend.rs:1144】 | ნაგულისხმევი ყველა წარმოების ბინარებისთვის. |
| სურვილისამებრ GPU აჩქარება | `fastpq_prover/fastpq-gpu` | რთავს CUDA/მეტალის ბირთვებს CPU-ს ავტომატური დაბრუნებით.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | მასპინძლები მხარდაჭერილი ამაჩქარებლებით. |

## მშენებლობის პროცედურა
1. **მხოლოდ CPU-ის აშენება **
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   წარმოების backend შედგენილია ნაგულისხმევად; არ არის საჭირო დამატებითი ფუნქციები.

2. **GPU ჩართულია build (სურვილისამებრ)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU მხარდაჭერისთვის საჭიროა SM80+ CUDA ხელსაწყოთა ნაკრები `nvcc`-ით, რომელიც ხელმისაწვდომია მშენებლობის დროს.【crates/fastpq_prover/Cargo.toml:11】

3. **თვითტესტი **
   ```bash
   cargo test -p fastpq_prover
   ```
   გაუშვით ეს ერთხელ გამოშვების აწყობაში, რათა დაადასტუროთ Stage6 გზა შეფუთვამდე.

### ლითონის ხელსაწყოების ჯაჭვის მომზადება (macOS)
1. დააინსტალირეთ Metal ბრძანების ხაზის ხელსაწყოები აშენებამდე: `xcode-select --install` (თუ CLI ინსტრუმენტები აკლია) და `xcodebuild -downloadComponent MetalToolchain` GPU ინსტრუმენტთა ჯაჭვის მისაღებად. build სკრიპტი პირდაპირ იწვევს `xcrun metal`/`xcrun metallib`-ს და სწრაფად იშლება, თუ ბინარები არ არის.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121【
2. CI-მდე მილსადენის დასადასტურებლად, შეგიძლიათ ასახოთ build სკრიპტი ადგილობრივად:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   როდესაც ეს წარმატებას მიაღწევს, აშენება გამოსცემს `FASTPQ_METAL_LIB=<path>`; გაშვების დრო კითხულობს ამ მნიშვნელობას metallib-ის დეტერმინისტულად ჩატვირთვისთვის.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. დააყენეთ `FASTPQ_SKIP_GPU_BUILD=1` ჯვარედინი კომპილაციისას Metal toolchain-ის გარეშე; build ბეჭდავს გაფრთხილებას და დამგეგმავი რჩება პროცესორის გზაზე.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. კვანძები ავტომატურად ბრუნდება CPU-ში, თუ Metal მიუწვდომელია (გამოტოვებული ჩარჩო, მხარდაჭერილი GPU ან ცარიელი `FASTPQ_METAL_LIB`); build-ის სკრიპტი ასუფთავებს env var-ს და დამგეგმავი ჩაწერს რეჟიმს.### გამოშვების ჩამონათვალი (სტადია6)
შეინახეთ FASTPQ-ის გამოშვების ბილეთი დაბლოკილი, სანამ ქვემოთ მოცემული ყველა ელემენტი არ დასრულებული და დართული არ არის.

1. **ქვემეორე მტკიცებულების მეტრიკა** — შეამოწმეთ ახლად გადაღებული `fastpq_metal_bench_*.json` და
   დაადასტურეთ `benchmarks.operations` ჩანაწერი, სადაც `operation = "lde"` (და სარკისებური
   `report.operations` ნიმუში) იუწყება `gpu_mean_ms ≤ 950` 20000 მწკრივი დატვირთვისთვის (32768 შეფუთული
   რიგები). ჭერის მიღმა გადაღებები საჭიროებს ხელახლა გაშვებას საკონტროლო სიის ხელმოწერამდე.
2. **ხელმოწერილი მანიფესტი** — გაუშვით
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   ასე რომ, გათავისუფლების ბილეთს აქვს როგორც მანიფესტი, ასევე მისი მოწყვეტილი ხელმოწერა
   (`artifacts/fastpq_bench_manifest.sig`). მიმომხილველები მანამდე ამოწმებენ დაიჯესტის/ხელმოწერის წყვილს
   გამოშვების ხელშეწყობა.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 მატრიცის მანიფესტი (აშენებული
   `scripts/fastpq/capture_matrix.sh`-ის მეშვეობით) უკვე შიფრავს 20 ათასი რიგის იატაკს და
   რეგრესიის გამართვა.
3. **მტკიცებულებების დანართები** — ატვირთეთ Metal ბენჩმარკი JSON, stdout log (ან Instruments trace),
   CUDA/Metal manifest-ის გამომავალი გამომავალი და მოწყვეტილი ხელმოწერა გამოშვების ბილეთზე. საკონტროლო სიის ჩანაწერი
   უნდა დაუკავშირდეს ყველა არტეფაქტს, პლუს საჯარო გასაღების თითის ანაბეჭდს, რომელიც გამოიყენება ამ აუდიტის ხელმოწერისთვის
   შეუძლია გადაამოწმოს გადამოწმების ნაბიჯი.【artifacts/fastpq_benchmarks/README.md:65】### ლითონის ვალიდაციის სამუშაო პროცესი
1. GPU-ზე ჩართული build-ის შემდეგ, დაადასტურეთ `FASTPQ_METAL_LIB` წერტილები `.metallib`-ზე (`echo $FASTPQ_METAL_LIB`), რათა გაშვების დრომ შეძლოს მისი დეტერმინისტულად ჩატვირთვა.【crates/fastpq_prover/build.rs:188】
2. გაუშვით პარიტეტული კომპლექტი GPU ზოლებით იძულებით:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. ბექენდი გამოიმუშავებს მეტალის ბირთვებს და დარეგისტრირებს დეტერმინისტულ CPU-ს, თუ გამოვლენა ვერ მოხერხდება.
3. გადაიღეთ საორიენტაციო ნიმუში საინფორმაციო დაფებისთვის:\
   იპოვნეთ შედგენილი ლითონის ბიბლიოთეკა (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   ექსპორტი `FASTPQ_METAL_LIB`-ით და გაუშვით\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  კანონიკური `fastpq-lane-balanced` პროფილი ახლა ავსებს ყველა გადაღებას 32,768 მწკრივამდე (2¹5), ასე რომ, JSON ატარებს `rows` და `padded_rows` Metal LDE შეყოვნებასთან ერთად; ხელახლა გაუშვით გადაღება, თუ `zero_fill` ან რიგის პარამეტრები აყენებს GPU LDE-ს 950ms (<1s) სამიზნეს AppleM-ის სერიის ჰოსტებზე. დაარქივეთ მიღებული JSON/log სხვა გამოშვების მტკიცებულებებთან ერთად; ღამის macOS სამუშაო ნაკადი ასრულებს იგივე გაშვებას და ატვირთავს მის არტეფაქტებს შედარებისთვის.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  როდესაც გჭირდებათ მხოლოდ პოსეიდონის ტელემეტრია (მაგ. Instruments კვალის ჩასაწერად), დაამატეთ `--operation poseidon_hash_columns` ზემოთ მოცემულ ბრძანებას; სკამი მაინც პატივს სცემს `FASTPQ_GPU=gpu`-ს, გამოსცემს `metal_dispatch_queue.poseidon` და მოიცავს ახალ `poseidon_profiles` ბლოკს, ასე რომ, გამოშვების ნაკრები ცალსახად ადასტურებს პოსეიდონის შეფერხებას.
  მტკიცებულება ახლა მოიცავს `zero_fill.{bytes,ms,queue_delta}` პლუს `kernel_profiles` (თითო ბირთვზე
  დაკავებულობა, სავარაუდო გბ/წმ და ხანგრძლივობის სტატისტიკა) ასე რომ, GPU ეფექტურობის გრაფიკის გარეშე
  ნედლეული კვალის ხელახალი დამუშავება და `twiddle_cache` ბლოკი (დარტყმები/გამოტოვება + `before_ms`/`after_ms`), რომელიც
  ადასტურებს, რომ ქეშირებული twiddle ატვირთვები მოქმედებს. `--trace-dir` ხელახლა უშვებს აღკაზმულობას ქვეშ
  `xcrun xctrace record` და
  ინახავს დროის შტამპს `.trace` ფაილს JSON-თან ერთად; თქვენ მაინც შეგიძლიათ მიაწოდოთ შეკვეთა
  `--trace-output` (სურვილისამებრ `--trace-template` / `--trace-seconds`) გადაღებისას
  მორგებული მდებარეობა/თარგი. JSON ჩაწერს `metal_trace_{template,seconds,output}` აუდიტისთვის.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】ყოველი გადაღების შემდეგ `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json`, ასე რომ, პუბლიკაცია ატარებს ჰოსტის მეტამონაცემებს (ახლა `metadata.metal_trace` ჩათვლით) Grafana დაფის/გაფრთხილების ნაკრებისთვის (`dashboards/grafana/fastpq_acceleration.json`, IVM). მოხსენება ახლა ატარებს `speedup` ობიექტს თითო ოპერაციაზე (`speedup.ratio`, `speedup.delta_ms`), სახვევის ამწეები `zero_fill_hotspots` (ბაიტები, შეყოვნება, მიღებული გბ/წმ და ლითონის ბრტყელი რიგის დელტა107 მრიცხველები10X). `benchmarks.kernel_summary`, ინარჩუნებს `twiddle_cache` ბლოკს ხელუხლებლად, აკოპირებს ახალ `post_tile_dispatches` ბლოკს/შეჯამებას, რათა მიმომხილველებმა დაამტკიცონ, რომ მრავალგადასასვლელი ბირთვი გაშვებული იყო გადაღების დროს და ახლა აჯამებს Poseidon microbench მტკიცებულებებს I08-ში. scalar-vs-default შეყოვნება ნედლი ანგარიშის გადახედვის გარეშე. მანიფესტის კარიბჭე კითხულობს იმავე ბლოკს და უარყოფს GPU-ს მტკიცებულების პაკეტებს, რომლებიც გამოტოვებს მას, აიძულებს ოპერატორებს განაახლონ გადაღებები, როდესაც კრამიტის შემდგომი გზა გამოტოვებულია ან არასწორი კონფიგურაცია.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq /wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  Poseidon2 Metal kernel იზიარებს იგივე ღილაკებს: `FASTPQ_METAL_POSEIDON_LANES` (32–256, სიმძლავრე ორი) და `FASTPQ_METAL_POSEIDON_BATCH` (1–32 მდგომარეობა თითო ზოლზე) საშუალებას გაძლევთ დაამაგროთ გაშვების სიგანე და თითო ზოლზე მუშაობა ხელახლა აშენების გარეშე; ჰოსტი აგზავნის ამ მნიშვნელობებს `PoseidonArgs`-ის მეშვეობით ყოველი გაგზავნის წინ. ნაგულისხმევად, გაშვების დრო ამოწმებს `MTLDevice::{is_low_power,is_headless,location}`-ს, რათა მიკერძოდეს დისკრეტული GPU-ები VRAM დონის გაშვებისკენ (`256×24`, როდესაც ≥48GiB არის მოხსენებული, `256×20` 32GiB-ზე, I100082000, წინააღმდეგ შემთხვევაში რჩება დაბალი `256×8` (და უფრო ძველი 128/64 ზოლის ნაწილები ინარჩუნებს 8/6 მდგომარეობას თითო ზოლზე), ასე რომ, ოპერატორების უმეტესობას არასოდეს სჭირდება env vars ხელით დაყენება. `fastpq_metal_bench` ახლა ხელახლა ახორციელებს თავს `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}`-ის ქვეშ და ასხივებს `poseidon_microbench` ბლოკს, რომელიც ჩაწერს გაშვების ორივე პროფილს პლუს გაზომილ სიჩქარეს სკალარული ზოლის მიმართ, ასე რომ, გამოშვების პაკეტებმა შეიძლება დაამტკიცონ, რომ ახალი ბირთვი რეალურად იკლებს I1808NI000, და მოიცავს `poseidon_pipeline` ბლოკი, ასე რომ, Stage7-ის მტკიცებულება ასახავს ბლოკის სიღრმის/გადახურვის ღილაკებს დაკავების ახალ საფეხურებთან ერთად. დატოვეთ env დაყენებული ნორმალური გაშვებისთვის; აღკაზმულობა ავტომატურად მართავს ხელახლა შესრულებას, აღრიცხავს წარუმატებლობებს, თუ ბავშვის გადაღება ვერ გაშვება და დაუყოვნებლივ გადის, როდესაც დაყენებულია `FASTPQ_GPU=gpu`, მაგრამ GPU-ს უკანა ნაწილი არ არის ხელმისაწვდომი, ასე რომ CPU-ს ჩუმი ჩანაცვლება არასოდეს შემეპარება პერფში. არტეფაქტები.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】შეფუთვა უარყოფს პოსეიდონის გადაღებებს, რომლებსაც აკლია `metal_dispatch_queue.poseidon` დელტა, საერთო `column_staging` მრიცხველები, ან `poseidon_profiles`/`poseidon_microbench`, ამიტომ ოპერატორებმა უნდა განაახლონ ან დაამტკიცონ, რომ ნებისმიერი მტკიცებულების ბლოკი უნდა განაახლონ ან დაამტკიცონ. scalar-vs-default speedup.【scripts/fastpq/wrap_benchmark.py:732】 როცა გჭირდებათ დამოუკიდებელი JSON დაფებისთვის ან CI დელტაებისთვის, გაუშვით `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; დამხმარე იღებს როგორც შეფუთულ არტეფაქტებს, ასევე დაუმუშავებელ `fastpq_metal_bench*.json` გადაღებებს, ასხივებს `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` ნაგულისხმევ/სკალარული დროებით, მეტამონაცემების დარეგულირებით და ჩაწერილი სიჩქარით.【scripts/fastpq/export_poseidon_microbench.
  დაასრულეთ გაშვება `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`-ის შესრულებით, რათა Stage6 გამოშვების საკონტროლო სია აღასრულოს `<1 s` LDE ჭერი და გამოუშვას ხელმოწერილი მანიფესტი/დაიჯესტის პაკეტი, რომელიც მიეწოდება გამოშვებას. ბილეთი.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. გადაამოწმეთ ტელემეტრია გაშვებამდე: გადაახვიეთ Prometheus საბოლოო წერტილი (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) და შეამოწმეთ `telemetry::fastpq.execution_mode` ჟურნალები მოულოდნელი `resolved="cpu"`-ისთვის ჩანაწერები.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. დააფიქსირეთ CPU-ის სარეზერვო გზა განზრახ იძულებით (`FASTPQ_GPU=cpu` ან `zk.fastpq.execution_mode = "cpu"`), რათა SRE სათამაშო წიგნები დარჩეს დეტერმინისტულთან შესაბამისობაში ქცევა.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. არასავალდებულო რეგულირება: ნაგულისხმევად, ჰოსტი ირჩევს 16 ზოლს მოკლე ტრასებისთვის, 32 საშუალოდ და 64/128 ერთხელ `log_len ≥ 10/14`, დაჯდება 256-ზე, როდესაც `log_len ≥ 18`, და ახლა ინახავს საერთო მეხსიერების ფილას ოთხ ეტაპზე, ერთხელ I1001, ხოლო I1001, ერთხელ I181,001-ზე. 12/14/16 ეტაპები `log_len ≥ 18/20/22`-სთვის სამუშაოს დაწყებამდე კრამიტის შემდგომ ბირთვში. გაიარეთ `FASTPQ_METAL_FFT_LANES` (ორი სიმძლავრე 8-დან 256-მდე) და/ან `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) ექსპორტზე ზემოთ მოცემული ნაბიჯების შესრულებამდე, რათა გადალახოთ ეს ევრისტიკა. ორივე FFT/IFFT და LDE სვეტის სერიის ზომები გამომდინარეობს ძაფთა ჯგუფის გადაწყვეტილი სიგანისგან (≈2048 ლოგიკური ძაფები თითო დისპეტჩერზე, 32 სვეტზე დახურული და ახლა მცირდება 32→16→8→4→2→1, როგორც დომენის ზრდა) ხოლო მისი LDE დომენი ჯერ კიდევ en; დააყენეთ `FASTPQ_METAL_FFT_COLUMNS` (1–32) დეტერმინისტული FFT სერიის ზომის დასამაგრებლად და `FASTPQ_METAL_LDE_COLUMNS` (1–32), რომ გამოიყენოს იგივე უგულებელყოფა LDE დისპეჩერზე, როდესაც გჭირდებათ ბიტ-ბიტი შედარება ჰოსტებს შორის. LDE კრამიტის სიღრმე ასევე ასახავს FFT ევრისტიკას - `log₂ ≥ 18/20/22`-ით კვალი გადის მხოლოდ 12/10/8 საზიარო მეხსიერების ეტაპებს, სანამ ფართო პეპლებს გადასცემთ კრამიტის შემდგომ ბირთვს - და შეგიძლიათ ამ ლიმიტის გადალახვა Prometheus-ის საშუალებით. გაშვების დრო ანაწილებს ყველა მნიშვნელობას Metal kernel args-ში, ამაგრებს მხარდაუჭერელ გადაფარვას და აღრიცხავს გადაწყვეტილ მნიშვნელობებს, ასე რომ ექსპერიმენტები რჩება რეპროდუცირებადი metallib-ის აღდგენის გარეშე; საორიენტაციო JSON ასახავს როგორც გადაწყვეტილ ტუნინგს, ასევე მასპინძლის ნულოვანი შევსების ბიუჯეტს (`zero_fill.{bytes,ms,queue_delta}`), რომელიც აღბეჭდილია LDE სტატისტიკის საშუალებით, ასე რომ, რიგის დელტაები პირდაპირ მიბმულია თითოეულ გადაღებასთან და ახლა ამატებს `column_staging` ბლოკს (პარტიები გაბრტყელებულია, გაბრტყელებულია ჰოსპინძლები, გაბრტყელებულია) ორმაგი ბუფერული მილსადენის მიერ შემოტანილი გადახურვა. როდესაც GPU უარს ამბობს ნულოვანი შევსების ტელემეტრიის შესახებ შეტყობინებაზე, აღკაზმულობა ახლა ასინთეზებს განმსაზღვრელ ვადებს მასპინძლის მხარის ბუფერიდან და შეჰყავს მას `zero_fill` ბლოკში, ასე რომ, მტკიცებულება არასოდეს გაიგზავნება გარეშე სფეროში.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. მრავალ რიგის გაგზავნა ავტომატურია დისკრეტულ Mac-ებზე: როდესაც `Device::is_low_power()` დააბრუნებს false-ს ან ლითონის მოწყობილობა იტყობინება სლოტ/გარე მდებარეობის შესახებ, მასპინძელი წარმოქმნის ორ `MTLCommandQueue`-ს, მხოლოდ გულშემატკივრები გამოდიან მას შემდეგ, რაც სამუშაო დატვირთვა ახორციელებს ≥16 სვეტს (მრგვალდება გულშემატკივრებით) და სვეტების მიხედვით. გრძელი კვალი ინარჩუნებს ორივე GPU ზოლს დატვირთული დეტერმინიზმის გარეშე. გააუქმეთ პოლიტიკა `FASTPQ_METAL_QUEUE_FANOUT`-ით (1–4 რიგები) და `FASTPQ_METAL_COLUMN_THRESHOLD` (სვეტების მინიმალური ჯამური გამორთვამდე) ყოველთვის, როცა დაგჭირდებათ ხელახალი გადაღებები მანქანებში; პარიტეტის ტესტები აიძულებს ამ გადაფარვას, რათა მრავალ GPU Mac-ები დარჩეს დაფარული და გადაწყვეტილი fan-out/ზღვრული დარეგისტრირებული იყოს რიგის სიღრმის გვერდით ტელემეტრია.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### მტკიცებულება არქივისთვის
| არტეფაქტი | გადაღება | შენიშვნები |
|----------|---------|-------|
| `.metallib` პაკეტი | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` და `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, რასაც მოჰყვება `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` და `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | ადასტურებს, რომ Metal CLI/ინსტრუმენტების ჯაჭვი იყო დაინსტალირებული და შეიქმნა დეტერმინისტული ბიბლიოთეკა ამ კომიტისთვის.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| გარემოს სურათი | `echo $FASTPQ_METAL_LIB` აშენების შემდეგ; შეინარჩუნეთ აბსოლუტური გზა თქვენი გათავისუფლების ბილეთით. | ცარიელი გამომავალი ნიშნავს Metal იყო გამორთული; ღირებულების დოკუმენტების ჩაწერა, რომ GPU ზოლები ხელმისაწვდომი რჩება გადაზიდვის არტეფაქტზე.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU პარიტეტის ჟურნალი | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` და დაარქივეთ ფრაგმენტი, რომელიც შეიცავს `backend="metal"` ან შემცირების გაფრთხილებას. | აჩვენებს, რომ ბირთვები მუშაობს (ან დეტერმინისტულად უკან იხევს) სანამ აწყობთ build-ს.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| საორიენტაციო გამომავალი | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; შეფუთეთ და მოაწერეთ ხელი `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`-ით. | შეფუთული JSON ჩანაწერები `speedup.ratio`, `speedup.delta_ms`, FFT tuning, დატენილი რიგები (32,768), გამდიდრებული `zero_fill`/`kernel_profiles`, გაბრტყელებული50X0, I183 გაბრტყელებული50X0, I181I `metal_dispatch_queue.poseidon`/`poseidon_profiles` ბლოკავს (როდესაც გამოიყენება `--operation poseidon_hash_columns`), ხოლო კვალის მეტამონაცემები, ასე რომ GPU LDE საშუალო რჩება ≤950ms და Poseidon რჩება <1s; შეინახეთ როგორც ნაკრები, ასევე გენერირებული `.json.asc` ხელმოწერა გამოშვების ბილეთთან ერთად, რათა დაფებმა და აუდიტორებმა შეძლონ არტეფაქტის გადამოწმება ხელახლა გაშვების გარეშე სამუშაო დატვირთვები.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Bench manifest | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | ამოწმებს ორივე GPU-ს არტეფაქტს, ვერ ხერხდება, თუ LDE საშუალო არღვევს `<1 s` ჭერს, ჩაიწერს BLAKE3/SHA-256 დაჯესტებს და გამოსცემს ხელმოწერილ მანიფესტს, ასე რომ, გამოშვების საკონტროლო სია ვერ გადაინაცვლებს შემოწმების გარეშე მეტრიკა.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA პაკეტი | გაუშვით `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` SM80 ლაბორატორიის ჰოსტზე, გადაახვიეთ/მოწერეთ JSON `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`-ში (გამოიყენეთ `--label device_class=xeon-rtx-sm80`, რათა დაფამ შეარჩიოს სწორი კლასი), დაამატეთ გზა `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`-მდე და შეინახეთ `.json`/`.asc` წყვილდება ლითონის არტეფაქტთან მანიფესტის რეგენერაციამდე. რეგისტრირებული `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` ასახავს ზუსტ პაკეტების ფორმატის მოლოდინს აუდიტორებს.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm81
| ტელემეტრიული მტკიცებულება | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` პლუს `telemetry::fastpq.execution_mode` ჟურნალი, რომელიც გამოიცა გაშვებისას. | ადასტურებს Prometheus/OTEL-ის გამოვლენას `device_class="<matrix>", backend="metal"` (ან დაქვეითების ჟურნალი) ტრაფიკის ჩართვამდე.| CPU იძულებითი საბურღი | გაუშვით მოკლე პარტია `FASTPQ_GPU=cpu`-ით ან `zk.fastpq.execution_mode = "cpu"`-ით და აიღეთ შემცირების ჟურნალი. | ინახავს SRE წიგნებს გასწორებულ დეტერმინისტულ სარეზერვო ბილიკთან, იმ შემთხვევაში, თუ საჭირო იქნება უკან დაბრუნება შუა გამოშვებაში.
| კვალი აღბეჭდვა (სურვილისამებრ) | გაიმეორეთ პარიტეტის ტესტი `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …`-ით და შეინახეთ ემიტირებული დისპეტჩერიზაციის კვალი. | ინახავს დაკავებულობის/ნაკადის ჯგუფის მტკიცებულებებს შემდგომი პროფილირების მიმოხილვებისთვის განმეორებითი საორიენტაციო ნიშნების გარეშე.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

მრავალენოვანი `fastpq_plan.*` ფაილები მიუთითებს ამ საკონტროლო სიაზე, ასე რომ, დადგმისა და წარმოების ოპერატორები მიჰყვებიან იგივე მტკიცებულების ბილიკს.【docs/source/fastpq_plan.md:1】

## რეპროდუცირებადი შენობები
გამოიყენეთ მიმაგრებული კონტეინერის სამუშაო ნაკადი Stage6-ის რეპროდუცირებადი არტეფაქტების შესაქმნელად:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

დამხმარე სკრიპტი აშენებს `rust:1.88.0-slim-bookworm` ინსტრუმენტთა ჯაჭვის სურათს (და `nvidia/cuda:12.2.2-devel-ubuntu22.04` GPU-სთვის), აწარმოებს build-ს კონტეინერის შიგნით და წერს `manifest.json`, `sha256s.txt` და შედგენილ ბინარებს მიზნად. დირექტორია.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

გარემო უგულებელყოფს:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – დაამაგრეთ აშკარა Rust-ის ბაზა/ტეგი.
- `FASTPQ_CUDA_IMAGE` – შეცვალეთ CUDA ბაზა GPU არტეფაქტების წარმოებისას.
- `FASTPQ_CONTAINER_RUNTIME` – დააწესეთ კონკრეტული გაშვების დრო; ნაგულისხმევი `auto` ცდილობს `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – მძიმით გამოყოფილი უპირატესობის შეკვეთა გაშვების დროის ავტომატური გამოვლენისთვის (ნაგულისხმევად არის `docker,podman,nerdctl`).

## კონფიგურაციის განახლებები
1. დააყენეთ გაშვების შესრულების რეჟიმი თქვენს TOML-ში:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   მნიშვნელობა გაანალიზებულია `FastpqExecutionMode`-ის მეშვეობით და გადადის ბექენდში გაშვებისას.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. აუცილებლობის შემთხვევაში გაშვებისას გადააცილეთ:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI უგულებელყოფს მოგვარებული კონფიგურაციის მუტაციას კვანძის ჩატვირთვამდე.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. დეველოპერებს შეუძლიათ დროებით აიძულონ ამოცნობა კონფიგურაციებზე შეხების გარეშე ექსპორტით
   `FASTPQ_GPU={auto,cpu,gpu}` ბინარის გაშვებამდე; გადაფარვა არის შესული და მილსადენი
   კვლავ რჩება გადაწყვეტილ რეჟიმში.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## გადამოწმების სია
1. ** გაშვების ჟურნალი **
   - ველით `FASTPQ execution mode resolved` სამიზნე `telemetry::fastpq.execution_mode`-ით
     `requested`, `resolved` და `backend` ეტიკეტები.【crates/fastpq_prover/src/backend.rs:208】
   - GPU-ს ავტომატური გამოვლენისას მეორადი ჟურნალი `fastpq::planner`-დან იტყობინება საბოლოო ზოლს.
   - ლითონის მასპინძლები ზედაპირზე `backend="metal"`, როდესაც metallib წარმატებით იტვირთება; თუ კომპილაცია ან ჩატვირთვა ვერ მოხერხდა, build სკრიპტი გამოსცემს გაფრთხილებას, ასუფთავებს `FASTPQ_METAL_LIB` და დამგეგმავი ჩაწერს `GPU acceleration unavailable`-ზე დარჩენამდე CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover:4/fastpq_prover/rs:42. **Prometheus მეტრიკა**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   მრიცხველი იზრდება `record_fastpq_execution_mode`-ის საშუალებით (ახლა იარლიყით
   `{device_class,chip_family,gpu_kind}`) როდესაც კვანძი წყვეტს მის შესრულებას
   რეჟიმი.【crates/iroha_telemetry/src/metrics.rs:8887】
   - ლითონის დაფარვისთვის დაადასტურეთ
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     იზრდება თქვენი განლაგების დაფებთან ერთად.【crates/iroha_telemetry/src/metrics.rs:5397】
   - MacOS კვანძები, რომლებიც შედგენილია `irohad --features fastpq-gpu`-ით, დამატებით ავლენს
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     და
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` ასე რომ Stage7 დაფები
     შეუძლია თვალყური ადევნოს სამუშაო ციკლს და რიგს სათავეს ცოცხალი Prometheus ნაკაწრებიდან.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **ტელემეტრიის ექსპორტი**
   - OTEL builds ასხივებს `fastpq.execution_mode_resolutions_total` იგივე ეტიკეტებით; უზრუნველყოს შენი
     დაფები ან გაფრთხილებები უყურებენ მოულოდნელ `resolved="cpu"`-ს, როდესაც GPU უნდა იყოს აქტიური.

4. ** საღი აზრის დადასტურება/დამოწმება **
   - გაუშვით მცირე პარტია `iroha_cli`-ის ან ინტეგრაციის აღკაზმულობის მეშვეობით და დაადასტურეთ მტკიცებულებების შემოწმება
     იგივე პარამეტრებით შედგენილი თანატოლი.

## პრობლემების მოგვარება
- **გადაწყვეტილი რეჟიმი რჩება CPU GPU ჰოსტებზე** — შეამოწმეთ, რომ ორობითი იყო აგებული
  `fastpq_prover/fastpq-gpu`, CUDA ბიბლიოთეკები ჩამტვირთველის გზაზეა და `FASTPQ_GPU` არ აიძულებს
  `cpu`.
- **მეტალი მიუწვდომელია Apple Silicon-ზე** — შეამოწმეთ, რომ CLI ინსტრუმენტები დაინსტალირებულია (`xcode-select --install`), ხელახლა გაუშვით `xcodebuild -downloadComponent MetalToolchain` და დარწმუნდით, რომ ნაგებობამ წარმოქმნა არა ცარიელი `FASTPQ_METAL_LIB` გზა; ცარიელი ან დაკარგული მნიშვნელობა გამორთავს ბექენდს დიზაინის მიხედვით.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` შეცდომები** — დარწმუნდით, რომ როგორც პროვერტმა, ასევე შემმოწმებელმა გამოიყენონ ერთი და იგივე კანონიკური კატალოგი
  გამოსხივებული `fastpq_isi`; არ ემთხვევა ზედაპირს, როგორც `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **CPU მოულოდნელი უკან დაბრუნება** — შეამოწმეთ `cargo tree -p fastpq_prover --features` და
  დაადასტურეთ, რომ `fastpq_prover/fastpq-gpu` არის GPU-ს კონსტრუქციებში; შეამოწმეთ `nvcc`/CUDA ბიბლიოთეკები ძიების გზაზე.
- ** ტელემეტრიის მრიცხველი აკლია** — შეამოწმეთ, რომ კვანძი დაიწყო `--features telemetry`-ით (ნაგულისხმევი)
  და რომ OTEL ექსპორტი (თუ ჩართულია) მოიცავს მეტრულ მილსადენს.【crates/iroha_telemetry/src/metrics.rs:8887】

## სარეზერვო პროცედურა
განმსაზღვრელი ჩანაცვლების ველი ამოღებულია. თუ რეგრესია მოითხოვს უკან დაბრუნებას,
გადააყენეთ ადრე ცნობილი კარგი გამოშვების არტეფაქტები და გამოიკვლიეთ Stage6-ის ხელახლა გამოცემამდე
ბინარები. დაარეგისტრირეთ ცვლილებების მართვის გადაწყვეტილება და დარწმუნდით, რომ წინსვლა დასრულდება მხოლოდ ამის შემდეგ
რეგრესია გასაგებია.

3. ტელემეტრიის მონიტორინგი, რათა დარწმუნდეთ, რომ `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` ასახავს მოსალოდნელს
   ჩანაცვლების აღსრულება.

## ტექნიკის საბაზისო ხაზი
| პროფილი | CPU | GPU | შენიშვნები |
| ------- | --- | --- | ----- |
| მითითება (სტადია6) | AMD EPYC7B12 (32 ბირთვი), 256 გიბ RAM | NVIDIA A10040GB (CUDA12.2) | 20000 მწკრივი სინთეტიკური პარტიები უნდა შეავსონ ≤1000 ms.【docs/source/fastpq_plan.md:131】 |
| მხოლოდ CPU-ზე | ≥32 ფიზიკური ბირთვი, AVX2 | – | ველით ~0,9–1,2 წმ 20000 მწკრივზე; შეინახეთ `execution_mode = "cpu"` დეტერმინიზმისთვის. |## რეგრესიის ტესტები
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU ჰოსტებზე)
- სურვილისამებრ ოქროს სამაგრის შემოწმება:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

დააფიქსირეთ ნებისმიერი გადახრები ამ საკონტროლო სიიდან თქვენს ოპერაციულ წიგნში და განაახლეთ `status.md` შემდეგ
მიგრაციის ფანჯარა დასრულებულია.