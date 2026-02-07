---
lang: hy
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ Արտադրության միգրացիայի ուղեցույց

Այս runbook նկարագրում է, թե ինչպես վավերացնել Stage6 արտադրության FASTPQ պրովերը:
Դետերմինիստական ​​տեղապահը հեռացվել է որպես այս միգրացիոն ծրագրի մաս:
Այն լրացնում է փուլային պլանը `docs/source/fastpq_plan.md`-ում և ենթադրում է, որ դուք արդեն հետևում եք
աշխատանքային տարածքի կարգավիճակը `status.md`-ում:

## Հանդիսատես և շրջանակ
- Վավերացնող օպերատորները թողարկում են արտադրության պրովերը բեմականացման կամ հիմնական ցանցի միջավայրերում:
- Ազատ արձակեք ինժեներներին, որոնք ստեղծում են երկուական կամ բեռնարկղեր, որոնք կուղարկվեն արտադրության հետին պլանով:
- SRE/դիտորդական խմբերը միացնում են նոր հեռաչափական ազդանշաններ և ահազանգում:

Շրջանակից դուրս. Kotodama պայմանագրի հեղինակում և IVM ABI փոփոխություններ (տես `docs/source/nexus.md`
կատարման մոդել):

## Խաղարկային մատրիցա
| Ճանապարհ | Բեռների առանձնահատկությունները միացնելու համար | Արդյունք | Երբ օգտագործել |
| ---- | ----------------------- | ------ | ----------- |
| Արտադրության պրովեր (լռելյայն) | _ոչ մի_ | Stage6 FASTPQ backend FFT/LDE պլանավորմամբ և DEEP-FRI խողովակաշարով:【crates/fastpq_prover/src/backend.rs:1144】 | Կանխադրված բոլոր արտադրական երկուականների համար: |
| Ընտրովի GPU արագացում | `fastpq_prover/fastpq-gpu` | Միացնում է CUDA/Մետաղական միջուկները՝ CPU-ի ավտոմատ հետադարձով:【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Հոսթներ՝ աջակցվող արագացուցիչներով: |

## Կառուցման կարգը
1. **Միայն պրոցեսորի կառուցում**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   Արտադրության ֆոնդը կազմվում է լռելյայնորեն. ոչ մի լրացուցիչ հնարավորություն չի պահանջվում:

2. **GPU-ով միացված կառուցում (ըստ ցանկության)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU-ի աջակցությունը պահանջում է SM80+ CUDA գործիքակազմ՝ `nvcc`-ով, որը հասանելի է կառուցման ընթացքում:【crates/fastpq_prover/Cargo.toml:11】

3. **Ինքնաթեստեր**
   ```bash
   cargo test -p fastpq_prover
   ```
   Գործարկեք սա մեկ անգամ մեկ թողարկման համար՝ փաթեթավորումից առաջ Stage6 ուղին հաստատելու համար:

### Մետաղական գործիքների շղթայի պատրաստում (macOS)
1. Տեղադրեք Metal հրամանի տող գործիքները կառուցելուց առաջ՝ `xcode-select --install` (եթե CLI գործիքները բացակայում են) և `xcodebuild -downloadComponent MetalToolchain`՝ GPU գործիքների շղթան բերելու համար: Կառուցման սկրիպտը ուղղակիորեն կանչում է `xcrun metal`/`xcrun metallib` և արագ կխափանվի, եթե երկուականները բացակայեն:【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121【
2. Խողովակաշարը CI-ից առաջ վավերացնելու համար դուք կարող եք արտացոլել կառուցման սցենարը տեղական մակարդակում՝
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Երբ դա հաջողվի, կառուցումը արտանետում է `FASTPQ_METAL_LIB=<path>`; գործարկման ժամանակը կարդում է այդ արժեքը՝ դետերմինիստիկ կերպով բեռնելու համար:【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Սահմանել `FASTPQ_SKIP_GPU_BUILD=1` առանց մետաղական գործիքների շղթայի խաչմերուկ կազմելիս; build-ը նախազգուշացում է տպում, և պլանավորողը մնում է պրոցեսորի ուղու վրա։【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Հանգույցները ավտոմատ կերպով վերադառնում են պրոցեսոր, եթե Metal-ը հասանելի չէ (բացակայում է շրջանակը, չաջակցվող GPU կամ դատարկ `FASTPQ_METAL_LIB`); build script-ը մաքրում է env var-ը, իսկ պլանավորողը գրանցում է իջեցումը:【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:### Թողարկման ստուգաթերթ (6-րդ փուլ)
Պահպանեք FASTPQ-ի թողարկման տոմսն արգելափակված, մինչև ստորև բերված յուրաքանչյուր տարր ավարտվի և կցվի:

1. **Ենթերկրորդ ապացույցի չափումներ** — Ստուգեք նոր գրավված `fastpq_metal_bench_*.json` և
   հաստատեք `benchmarks.operations` մուտքը, որտեղ `operation = "lde"` (և հայելային
   `report.operations` նմուշ) հաղորդում է `gpu_mean_ms ≤ 950` 20000 տող աշխատանքային ծանրաբեռնվածության համար (32768 լիցքավորված
   շարքեր): Առաստաղից դուրս նկարները պահանջում են կրկնություններ՝ նախքան ստուգաթերթի ստորագրումը:
2. **Ստորագրված մանիֆեստ** — Վազել
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   ուստի թողարկման տոմսը կրում է և՛ մանիֆեստը, և՛ դրա անջատված ստորագրությունը
   (`artifacts/fastpq_bench_manifest.sig`): Վերանայողները նախապես ստուգում են ամփոփում/ստորագրություն զույգը
   թողարկման խթանում:【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 Մատրիցային մանիֆեստը (կառուցված
   `scripts/fastpq/capture_matrix.sh`-ի միջոցով) արդեն կոդավորում է 20 հազար շարքի հատակը և
   ռեգրեսիայի վրիպազերծում:
3. **Ապացույցների հավելվածներ** — Վերբեռնեք Metal չափանիշները JSON, stdout log (կամ Instruments trace),
   CUDA/Metal մանիֆեստի արդյունքները և անջատված ստորագրությունը թողարկման տոմսին: Ստուգաթերթի մուտքագրում
   պետք է կապվի բոլոր արտեֆակտների հետ, գումարած հանրային բանալու մատնահետքը, որն օգտագործվում է ներքևում գտնվող աուդիտների ստորագրման համար
   կարող է վերարտադրել ստուգման քայլը:【artifacts/fastpq_benchmarks/README.md:65】### Մետաղական վավերացման աշխատանքային ընթացք
1. GPU-ով միացված կառուցումից հետո հաստատեք `FASTPQ_METAL_LIB` կետերը `.metallib`-ում (`echo $FASTPQ_METAL_LIB`), որպեսզի գործարկման ժամանակը կարողանա այն դետերմինիստականորեն բեռնել:【crates/fastpq_prover/build.rs:188】
2. Գործարկեք հավասարաչափ փաթեթը GPU գծերով, որոնք ստիպված են միացնել.
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Backend-ը կգործադրի մետաղական միջուկները և գրանցի CPU-ի որոշիչ հետադարձ վերադարձ, եթե հայտնաբերումը ձախողվի:【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Վերցրեք հենանիշի նմուշ վահանակների համար.
   գտնել կազմված մետաղական գրադարանը (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   արտահանեք այն `FASTPQ_METAL_LIB`-ի միջոցով և գործարկեք\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  Կանոնական `fastpq-lane-balanced` պրոֆիլն այժմ լրացնում է յուրաքանչյուր նկարահանում մինչև 32768 տող (2¹5), այնպես որ JSON-ը կրում է և՛ `rows`, և՛ `padded_rows`՝ մետաղական LDE հետաձգման հետ միասին; վերագործարկեք նկարը, եթե `zero_fill`-ը կամ հերթի կարգավորումները մղում են GPU LDE-ն ավելի քան 950ms (<1s) թիրախը AppleM-ի սերիայի հոսթերներում: Արխիվացրեք ստացված JSON/log-ը թողարկման այլ ապացույցների հետ մեկտեղ. գիշերային macOS-ի աշխատանքային հոսքը կատարում է նույն գործարկումը և վերբեռնում է իր արտեֆակտները համեմատության համար:【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Երբ Ձեզ անհրաժեշտ է միայն Poseidon-ի հեռաչափություն (օրինակ՝ Instruments-ի հետքը գրանցելու համար), ավելացրեք `--operation poseidon_hash_columns` վերը նշված հրամանին. նստարանը դեռևս կհարգի `FASTPQ_GPU=gpu`-ը, կթողարկի `metal_dispatch_queue.poseidon` և կներառի նոր `poseidon_profiles` բլոկը, որպեսզի թողարկման փաթեթը հստակորեն փաստի Պոսեյդոնի խցանման մասին:
  Ապացույցներն այժմ ներառում են `zero_fill.{bytes,ms,queue_delta}` գումարած `kernel_profiles` (մեկ միջուկի համար
  զբաղվածությունը, գնահատված ԳԲ/վ և տևողության վիճակագրությունը), այնպես որ GPU-ի արդյունավետությունը կարելի է գծագրել առանց
  չմշակված հետքերի վերամշակում և `twiddle_cache` բլոկ (հարվածներ/բաց թողած + `before_ms`/`after_ms`), որը
  ապացուցում է, որ քեշավորված twiddle վերբեռնումները ուժի մեջ են: `--trace-dir`-ը վերագործարկում է ամրագոտիը տակից
  `xcrun xctrace record` և
  պահում է ժամանակի դրոշմավորված `.trace` ֆայլը JSON-ի կողքին; դուք դեռ կարող եք պատվիրել
  `--trace-output` (ըստ ցանկության `--trace-template` / `--trace-seconds`) լուսանկարելիս
  հարմարեցված տեղադրություն/կաղապար: JSON-ը գրանցում է `metal_trace_{template,seconds,output}`՝ աուդիտի համար:【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Յուրաքանչյուր նկարահանման գործարկումից հետո `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json`, այնպես որ հրապարակումը կրում է հոսթի մետատվյալներ (այժմ՝ ներառյալ `metadata.metal_trace`) Grafana տախտակի/ազդարարման փաթեթի համար (`dashboards/grafana/fastpq_acceleration.json`, IVM): Զեկույցն այժմ կրում է `speedup` օբյեկտ մեկ գործողության համար (`speedup.ratio`, `speedup.delta_ms`), փաթաթման վերելակները `zero_fill_hotspots` (բայթեր, ուշացում, ստացված ԳԲ/վ, և մետաղական հերթի դելտա I07 հաշվիչներ) `benchmarks.kernel_summary`, keeps the `twiddle_cache` block intact, copies the new `post_tile_dispatches` block/summary so reviewers can prove the multi-pass kernel ran during the capture, and now summarizes the Poseidon microbench evidence into `benchmarks.poseidon_microbench` so dashboards can quote the scalar-vs-default latency՝ առանց չմշակված հաշվետվությունը վերանայելու: Մանիֆեստի դարպասը կարդում է նույն բլոկը և մերժում է GPU-ի ապացույցների փաթեթները, որոնք բաց են թողնում այն՝ ստիպելով օպերատորներին թարմացնել նկարները, երբ երեսպատման ուղին բաց է թողնվում կամ սխալ կազմաձևված:【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq /wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  Poseidon2 Metal միջուկն ունի նույն կոճակները. `FASTPQ_METAL_POSEIDON_LANES` (32–256, երկուսի հզորություն) և `FASTPQ_METAL_POSEIDON_BATCH` (1–32 վիճակ յուրաքանչյուր գոտու համար) թույլ են տալիս ամրացնել մեկնարկի լայնությունը և յուրաքանչյուր գծի աշխատանքը՝ առանց վերակառուցման; հյուրընկալողն այդ արժեքները փոխանցում է `PoseidonArgs`-ի միջոցով յուրաքանչյուր առաքումից առաջ: Լռելյայնորեն գործարկման ժամանակը ստուգում է `MTLDevice::{is_low_power,is_headless,location}`՝ դիսկրետ GPU-ները կողմնորոշվելու դեպի VRAM մակարդակի գործարկումները (`256×24`, երբ ≥48GiB է հաղորդվում, `256×20` 32GiB-ում, I10008power, հակառակ դեպքում՝ I10080000) `256×8` (և ավելի հին 128/64 գծի մասերը կպչում են 8/6 վիճակների յուրաքանչյուր գծի վրա), ուստի օպերատորներից շատերը երբեք կարիք չունեն env vars-ը ձեռքով կարգավորելու։【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq99. `fastpq_metal_bench`-ն այժմ նորից գործարկվում է `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}`-ի ներքո և թողարկում է `poseidon_microbench` բլոկ, որը գրանցում է երկու գործարկման պրոֆիլները, գումարած չափված արագությունը սկալյար գծի նկատմամբ, այնպես որ թողարկման փաթեթները կարող են ապացուցել, որ նոր միջուկը իրականում փոքրանում է I1808NI000, և այն ներառում է I180870, `poseidon_pipeline` արգելափակում, որպեսզի Stage7-ի ապացույցը ֆիքսում է հատվածի խորության/համընկնման բռնակները նոր զբաղվածության մակարդակների կողքին: Նորմալ վազքի համար env-ը թողեք չկարգավորված; զրահը ինքնաբերաբար կառավարում է կրկնակի կատարումը, գրանցում է ձախողումները, եթե երեխայի ձայնագրումը չի կարող գործարկվել, և անմիջապես դուրս է գալիս, երբ `FASTPQ_GPU=gpu`-ը կարգավորված է, բայց GPU-ի հետնամասը հասանելի չէ, այնպես որ CPU-ի անաղմուկ հետադարձ կապերը երբեք չեն գաղտնազերծվում կատարման մեջ: artefacts.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】Փաթաթումը մերժում է Poseidon նկարահանումները, որոնք բացակայում են `metal_dispatch_queue.poseidon` դելտան, ընդհանուր `column_staging` հաշվիչները կամ `poseidon_profiles`/`poseidon_microbench` ապացույցների բլոկները, որպեսզի օպերատորները պետք է թարմացնեն կամ ապացուցեն, որ ցանկացած ապացույցի բլոկները պետք է թարմացնեն կամ ապացուցեն: scalar-vs-default speedup.【scripts/fastpq/wrap_benchmark.py:732】 Երբ վահանակների կամ CI դելտաների համար անհրաժեշտ է ինքնուրույն JSON, գործարկեք `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`; օգնականն ընդունում է և՛ փաթաթված արտեֆակտները, և՛ չմշակված `fastpq_metal_bench*.json` նկարները՝ արտանետելով `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` լռելյայն/սկալային ժամանակաչափերով, կարգավորելով մետատվյալները և գրանցված արագությունը:【scripts/fastpq/export_poseidon_microbench:
  Ավարտեք գործարկումը՝ գործարկելով `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`, որպեսզի Stage6 թողարկման ստուգացանկը ամրացնի `<1 s` LDE առաստաղը և թողարկի ստորագրված մանիֆեստի/դիզեստի փաթեթ, որը ուղարկվում է թողարկման հետ միասին: տոմս.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. Ստուգեք հեռաչափությունը մինչև թողարկումը. ոլորեք Prometheus վերջնակետը (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) և ստուգեք `telemetry::fastpq.execution_mode` տեղեկամատյանները անսպասելի `resolved="cpu"`-ի համար: գրառումներ։【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. Փաստաթղթավորեք պրոցեսորի հետադարձ ուղին՝ միտումնավոր ստիպելով այն (`FASTPQ_GPU=cpu` կամ `zk.fastpq.execution_mode = "cpu"`), որպեսզի SRE-ի գրքույկները համապատասխանեցվեն դետերմինիստականին: վարքագիծ.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. Լրացուցիչ թյունինգ. լռելյայն ընտրում է 16 գիծ՝ կարճ հետքերի համար, 32՝ միջին և 64/128՝ մեկ անգամ՝ `log_len ≥ 10/14`, վայրէջք կատարելով 256-ում, երբ `log_len ≥ 18`, և այժմ պահպանում է ընդհանուր հիշողության սալիկը՝ չորս փուլով, 40001 անգամ՝ հինգ փուլով, I1001 անգամ՝ հինգ փուլով: 12/14/16 փուլերը `log_len ≥ 18/20/22`-ի համար՝ նախքան աշխատանքը սալիկների տեղադրման միջուկը սկսելը: Արտահանեք `FASTPQ_METAL_FFT_LANES` (երկու հզորությունը 8-ի և 256-ի միջև) և/կամ `FASTPQ_METAL_FFT_TILE_STAGES`-ը (1–16)՝ նախքան վերը նշված քայլերն իրականացնելը՝ այդ էվրիստիկաները վերացնելու համար: Ե՛վ FFT/IFFT, և՛ LDE սյունակների խմբաքանակի չափերը բխում են շղթաների խմբի լուծված լայնությունից (≈2048 տրամաբանական շղթաներ մեկ ուղարկման համար, ծածկված 32 սյունակով և այժմ իջնում ​​են 32→16→8→4→2→1, քանի որ տիրույթը մեծանում է), մինչդեռ դրա տիրույթի տիրույթը դեռևս en. սահմանեք `FASTPQ_METAL_FFT_COLUMNS` (1–32)՝ ամրացնելու համար որոշիչ FFT խմբաքանակի չափը, և `FASTPQ_METAL_LDE_COLUMNS` (1–32)՝ կիրառելու նույն անտեսումը LDE դիսպետչերի վրա, երբ ձեզ հարկավոր են բիթ առ բիթ համեմատություններ հոսթերների միջև: LDE սալիկի խորությունը նույնպես արտացոլում է FFT էվրիստիկա. `log₂ ≥ 18/20/22`-ի հետքերն աշխատում են միայն 12/10/8 ընդհանուր հիշողության փուլերով՝ նախքան լայն թիթեռնիկները երեսպատման միջուկին հանձնելը, և դուք կարող եք խախտել այդ սահմանը Prometheus-ի միջոցով: Գործարկման ժամանակը փոխանցում է բոլոր արժեքները Metal kernel args-ի միջով, սեղմում է չաջակցվող վերադրումները և գրանցում է լուծված արժեքները, որպեսզի փորձերը մնան վերարտադրելի՝ առանց վերակառուցելու metallib; հենանիշը JSON-ը ներկայացնում է և՛ լուծված թյունինգը, և՛ հյուրընկալող զրոյական լիցքավորման բյուջեն (`zero_fill.{bytes,ms,queue_delta}`), որը նկարահանվել է LDE վիճակագրության միջոցով, այնպես որ հերթերի դելտաները ուղղակիորեն կապված են յուրաքանչյուր նկարահանման հետ, և այժմ ավելացնում է `column_staging` բլոկ (խմբաքանակները հարթեցված են, խմբաքանակները հարթեցված են, տափակել են հոսթինգները, սպասել_մաս, սպասել_մաս, սպասել_մասներ, կրկնակի բուֆերացված խողովակաշարի կողմից ներդրված համընկնումը: Երբ GPU-ն հրաժարվում է զեկուցել զրոյական լիցքավորման հեռաչափությունը, զրահը այժմ սինթեզում է դետերմինիստական ժամանակացույց՝ հյուրընկալող կողմի բուֆերի մաքրումից և ներարկում այն `zero_fill` բլոկի մեջ, այնպես որ թողարկեք ապացույցները երբեք առանց առաքման: դաշտ.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. Բազմահերթ ուղարկումն ավտոմատ է առանձին Mac-ներում. երբ `Device::is_low_power()`-ը վերադարձնում է false կամ մետաղական սարքը հայտնում է բնիկ/արտաքին տեղակայման մասին, հոսթն ակնարկում է երկու `MTLCommandQueue`, միայն օդափոխիչները դուրս են գալիս այն ժամանակ, երբ ծանրաբեռնվածությունն անցնում է ≥16 սյունակներով (կլորացված սյունակներով և սյունակներով կլորացված: երկար հետքերը զբաղված են պահում GPU-ի երկու երթուղիները՝ չվնասելով դետերմինիզմը: Անտեսեք կանոնը `FASTPQ_METAL_QUEUE_FANOUT`-ով (1–4 հերթ) և `FASTPQ_METAL_COLUMN_THRESHOLD`-ով (նվազագույն ընդհանուր սյունակները մինչև օդափոխիչի դուրս գալը) ամեն անգամ, երբ ձեզ անհրաժեշտ են վերարտադրվող նկարահանումներ մեքենաներում։ հավասարության թեստերը ստիպում են այդ անտեսումները, որպեսզի բազմա-GPU Mac-ները մնան ծածկված, և լուծված օդափոխության/շեմը գրանցվի հերթի խորության կողքին: հեռաչափություն.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### Ապացույցներ՝ արխիվային
| Արտեֆակտ | Գրավել | Ծանոթագրություններ |
|----------|---------|-------|
| `.metallib` փաթեթ | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` և `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, որին հաջորդում են `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` և `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`: | Ապացուցում է, որ Metal CLI/toolchain-ը տեղադրվել և արտադրվել է դետերմինիստական ​​գրադարան այս կատարման համար:【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Շրջակա միջավայրի պատկեր | `echo $FASTPQ_METAL_LIB` կառուցումից հետո; պահեք բացարձակ ճանապարհը ձեր թողարկման տոմսով: | Դատարկ ելքը նշանակում է, որ մետաղը անջատված է. գրանցելով արժեքային փաստաթղթերը, որ GPU-ի ուղիները հասանելի են մնում առաքման արտեֆակտի վրա:【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU-ի հավասարության մատյան | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` և արխիվացրեք այն հատվածը, որը պարունակում է `backend="metal"` կամ իջեցման նախազգուշացում: | Ցույց է տալիս, որ միջուկները աշխատում են (կամ հետ են ընկնում դետերմինիստականորեն) նախքան կառուցումը խթանելը:【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Հենանիշային ելք | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; փաթեթավորեք և ստորագրեք `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`-ի միջոցով: | Փաթաթված JSON-ը գրանցում է `speedup.ratio`, `speedup.delta_ms`, FFT թյունինգ, լիցքավորված տողեր (32,768), հարստացված `zero_fill`/`kernel_profiles`, հարթեցված 500000000134X, հարթեցված տարբերակը: `metal_dispatch_queue.poseidon`/`poseidon_profiles` արգելափակում է (երբ օգտագործվում է `--operation poseidon_hash_columns`), և հետքի մետատվյալները, այնպես որ GPU LDE-ի միջինը մնում է ≤950ms, իսկ Poseidon-ը մնում է <1s; Պահպանեք և՛ փաթեթը, և՛ ստեղծված `.json.asc` ստորագրությունը թողարկման տոմսի հետ, որպեսզի վահանակներն ու աուդիտորները կարողանան ստուգել արտեֆակտը՝ առանց կրկնելու: աշխատանքային բեռներ.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Նստարանային մանիֆեստ | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | Վավերացնում է երկու GPU արտեֆակտները, ձախողվում է, եթե LDE միջինը խախտում է `<1 s` առաստաղը, ձայնագրում է BLAKE3/SHA-256 բովանդակությունը և թողարկում ստորագրված մանիֆեստ, այնպես որ թողարկման ստուգաթերթը չի կարող առաջ շարժվել առանց ստուգման չափումներ.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA փաթեթ | Գործարկեք `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`-ը SM80 լաբորատոր հոսթի վրա, փաթեթավորեք/ստորագրեք JSON-ը `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`-ի մեջ (օգտագործեք `--label device_class=xeon-rtx-sm80`, որպեսզի վահանակներն ընտրեն ճիշտ դասը), ավելացրեք ուղին դեպի `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` և պահեք `.json`/`.asc` զուգորդվում է մետաղական արտեֆակտի հետ՝ նախքան մանիֆեստը վերականգնելը: Ստուգված `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`-ը ցույց է տալիս ճշգրիտ փաթեթի ձևաչափի աուդիտորները:【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80 |
| Հեռաչափության ապացույց | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` գումարած `telemetry::fastpq.execution_mode` գրանցամատյանը, որը թողարկվել է գործարկման ժամանակ: | Հաստատում է, որ Prometheus/OTEL-ը ցուցադրում է `device_class="<matrix>", backend="metal"`-ը (կամ իջեցման մատյան) նախքան երթևեկությունը միացնելը:| Հարկադիր պրոցեսորի փորված | Գործարկեք կարճ խմբաքանակ `FASTPQ_GPU=cpu` կամ `zk.fastpq.execution_mode = "cpu"`-ով և գրանցեք իջեցման մատյան: | Պահում է SRE մատյանները համահունչ դետերմինիստական ​​հետադարձ ուղու հետ, եթե թողարկման կեսին հետադարձ պահանջ լինի:【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Հետքի գրավում (ըստ ցանկության) | Կրկնեք հավասարության թեստը `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …`-ով և պահպանեք թողարկված դիսպետչերական հետքը: | Պահպանում է զբաղվածության/թելերի խմբի ապացույցները հետագա պրոֆիլային ակնարկների համար՝ առանց կրկնվող հենանիշերի։【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

`fastpq_plan.*` բազմալեզու ֆայլերը հղում են կատարում այս ստուգաթերթին, այնպես որ բեմականացման և արտադրության օպերատորները հետևում են նույն ապացույցների ուղուն:【docs/source/fastpq_plan.md:1】

## Վերարտադրվող շինություններ
Օգտագործեք ամրացված կոնտեյների աշխատանքային հոսքը՝ Stage6-ի վերարտադրվող արտեֆակտներ արտադրելու համար.

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Օգնական սկրիպտը կառուցում է `rust:1.88.0-slim-bookworm` գործիքների շղթայի պատկերը (և `nvidia/cuda:12.2.2-devel-ubuntu22.04`՝ GPU-ի համար), գործարկում է կոնտեյների ներսում և գրում `manifest.json`, `sha256s.txt` և հավաքագրված երկուականները թիրախում: գրացուցակ.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Շրջակա միջավայրը վերացնում է.
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – ամրացրեք բացահայտ Rust բազան/պիտակը:
- `FASTPQ_CUDA_IMAGE` – փոխեք CUDA բազան GPU արտեֆակտներ արտադրելիս:
- `FASTPQ_CONTAINER_RUNTIME` – ստիպել որոշակի գործարկման ժամանակ; լռելյայն `auto` փորձում է `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`:
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – գործարկման ժամանակի ավտոմատ հայտնաբերման համար ստորակետերով բաժանված նախապատվության կարգը (կանխադրված է `docker,podman,nerdctl`):

## Կազմաձևման թարմացումներ
1. Սահմանեք գործարկման ժամանակի կատարման ռեժիմը ձեր TOML-ում.
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   Արժեքը վերլուծվում է `FastpqExecutionMode`-ի միջոցով և մուտքագրվում է հետնամասում՝ գործարկման պահին:【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Անհրաժեշտության դեպքում վերացնել գործարկման ժամանակ.
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI-ն անտեսում է լուծված կազմաձևի փոփոխումը մինչև հանգույցի գործարկումը:【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Մշակողները կարող են ժամանակավորապես պարտադրել հայտնաբերումը, առանց կոնֆիգուրացիաների դիպչելու՝ արտահանելով
   `FASTPQ_GPU={auto,cpu,gpu}` նախքան երկուականը գործարկելը; վերագրանցումը գրանցված է և խողովակաշարը
   դեռ հայտնվում է լուծված ռեժիմով։【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Ստուգման ստուգաթերթ
1. ** Գործարկման տեղեկամատյաններ **
   - Սպասեք `FASTPQ execution mode resolved` թիրախ `telemetry::fastpq.execution_mode`-ից
     `requested`, `resolved` և `backend` պիտակներ:【crates/fastpq_prover/src/backend.rs:208】
   - GPU-ի ավտոմատ հայտնաբերման ժամանակ `fastpq::planner`-ի երկրորդական մատյանը հաղորդում է վերջին գիծը:
   - `backend="metal"` մետաղական հոսթինգի մակերեսը, երբ metallib-ը հաջողությամբ բեռնվում է; եթե կոմպիլյացիան կամ բեռնումը ձախողվում է, կառուցման սցենարը նախազգուշացում է թողարկում, ջնջում է `FASTPQ_METAL_LIB`-ը, և պլանավորողը գրանցում է `GPU acceleration unavailable` նախքան միացված մնալը: CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover:4/fastpq_prover/rs:2. **Prometheus չափումներ**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Հաշվիչը ավելացվում է `record_fastpq_execution_mode`-ի միջոցով (այժմ պիտակավորված է
   `{device_class,chip_family,gpu_kind}`), երբ հանգույցը լուծում է իր կատարումը
   ռեժիմ։【crates/iroha_telemetry/src/metrics.rs:8887】
   - Մետաղական ծածկույթի համար հաստատեք
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     ավելացումներ ձեր տեղակայման վահանակների հետ մեկտեղ:【crates/iroha_telemetry/src/metrics.rs:5397】
   - `irohad --features fastpq-gpu`-ով կազմված macOS հանգույցները լրացուցիչ բացահայտում են
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     և
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` այնպես որ Stage7 վահանակները
     կարող է հետևել Prometheus ուղիղ ցիկլի և հերթի գլխի տարածքին։【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Հեռաչափության արտահանում**
   - OTEL-ի կոնստրուկցիաները թողարկում են `fastpq.execution_mode_resolutions_total` նույն պիտակներով; ապահովել ձեր
     վահանակները կամ ազդանշանները դիտում են անսպասելի `resolved="cpu"`-ի համար, երբ GPU-ները պետք է ակտիվ լինեն:

4. **Ողջամտությունը ապացուցել/ստուգել**
   - Մի փոքր խմբաքանակ գործարկեք `iroha_cli`-ի կամ ինտեգրացիոն զրահի միջոցով և հաստատեք ապացույցները, որոնք հաստատում են
     նույն պարամետրերով կազմված peer.

## Անսարքությունների վերացում
- **Վերլուծված ռեժիմը մնում է պրոցեսորը GPU-ի հոստերում** — ստուգեք, որ երկուականը կառուցված է
  `fastpq_prover/fastpq-gpu`, CUDA գրադարանները բեռնիչի ուղու վրա են, և `FASTPQ_GPU`-ը չի ստիպում
  `cpu`.
- **Մետաղն անհասանելի է Apple Silicon-ում** — ստուգեք, որ CLI գործիքները տեղադրված են (`xcode-select --install`), նորից գործարկեք `xcodebuild -downloadComponent MetalToolchain` և համոզվեք, որ կառուցումը արտադրում է ոչ դատարկ `FASTPQ_METAL_LIB` ուղի; դատարկ կամ բացակայող արժեքը անջատում է հետին պլանը ըստ դիզայնի:【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` սխալներ** — համոզվեք, որ և՛ ապացուցողը, և՛ ստուգողը օգտագործում են նույն կանոնական կատալոգը
  թողարկված `fastpq_isi`-ի կողմից; անհամապատասխանության մակերեսը որպես `Error::UnknownParameter`:【crates/fastpq_prover/src/proof.rs:133】
- **CPU-ի անսպասելի հետադարձ** — ստուգեք `cargo tree -p fastpq_prover --features` և
  հաստատեք, որ `fastpq_prover/fastpq-gpu`-ն առկա է GPU-ի կառուցվածքներում. ստուգեք, որ `nvcc`/CUDA գրադարանները գտնվում են որոնման ուղու վրա:
- **Հեռաչափության հաշվիչը բացակայում է** — ստուգեք, որ հանգույցը սկսվել է `--features telemetry`-ով (կանխադրված)
  և որ OTEL արտահանումը (եթե միացված է) ներառում է մետրային խողովակաշարը:【crates/iroha_telemetry/src/metrics.rs:8887】

## Հետադարձ ընթացակարգ
Դետերմինիստական տեղապահի հետնամասը հեռացվել է: Եթե ռեգրեսիան պահանջում է հետադարձ,
վերաբաշխել նախկինում հայտնի լավ թողարկման արտեֆակտները և ուսումնասիրել նախքան Stage6-ի վերաթողարկումը
երկուականներ. Փաստաթղթավորեք փոփոխությունների կառավարման որոշումը և համոզվեք, որ առաջընթացը ավարտվում է միայն դրանից հետո
հետընթացը հասկացվում է.

3. Դիտարկեք հեռաչափությունը՝ ապահովելու համար, որ `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` արտացոլում է սպասվածը
   տեղապահի կատարումը:

## Սարքավորումների հիմքը
| Անձնագիր | CPU | GPU | Ծանոթագրություններ |
| ------- | --- | --- | ----- |
| Տեղեկանք (6-րդ փուլ) | AMD EPYC7B12 (32 միջուկ), 256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000 շարքով սինթետիկ խմբաքանակները պետք է լրացնեն ≤1000 մվ:【docs/source/fastpq_plan.md:131】 |
| Միայն պրոցեսորով | ≥32 ֆիզիկական միջուկ, AVX2 | – | Ակնկալվում է ~0,9–1,2 վրկ 20000 տողերի համար; պահել `execution_mode = "cpu"` դետերմինիզմի համար: |## Ռեգրեսիայի թեստեր
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU սերվերների վրա)
- Լրացուցիչ ոսկե ամրացման ստուգում.
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Փաստաթղթագրեք այս ստուգաթերթից ցանկացած շեղում ձեր օպերացիոն գրքում և թարմացրեք `status.md`-ը այն բանից հետո, երբ
միգրացիայի պատուհանն ավարտված է: