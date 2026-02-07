---
lang: kk
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! FASTPQ өндірісінің көшіру нұсқаулығы

Бұл жұмыс кітабы Stage6 өндірісінің FASTPQ проверін тексеру жолын сипаттайды.
Детерминирленген толтырғыш сервері осы тасымалдау жоспарының бөлігі ретінде жойылды.
Ол `docs/source/fastpq_plan.md` бағдарламасындағы кезеңдік жоспарды толықтырады және сіз бақылап отырасыз деп болжайды.
`status.md` ішіндегі жұмыс кеңістігі күйі.

## Аудитория және ауқым
- Сахналық немесе негізгі желі орталарында өндіріс проверін шығаратын валидатор операторлары.
- Өндірістік сервермен жеткізілетін екілік файлдарды немесе контейнерлерді жасайтын инженерлерді босатыңыз.
- SRE/бақылау топтары жаңа телеметриялық сигналдарды қосады және ескерту.

Қолдану аясынан тыс: Kotodama келісім-шарттың авторы және IVM ABI өзгерістері (үшін `docs/source/nexus.md` қараңыз).
орындау үлгісі).

## Мүмкіндік матрицасы
| Жол | | қосу үшін жүк мүмкіндіктері Нәтиже | Қашан пайдалану керек |
| ---- | --------------------------------- | ------ | ----------- |
| Өндірістік провер (әдепкі) | _жоқ_ | FFT/LDE жоспарлаушысы және DEEP-FRI құбыры бар Stage6 FASTPQ сервері.【crates/fastpq_prover/src/backend.rs:1144】 | Барлық өндіріс екілік файлдары үшін әдепкі. |
| Қосымша GPU жеделдету | `fastpq_prover/fastpq-gpu` | CUDA/Metal ядроларын автоматты түрде орталық процессорлық резервпен қосады.【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 | Қолдау көрсетілетін үдеткіштері бар хосттар. |

## Құру процедурасы
1. **Тек CPU құрастыру**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   Өндірістік сервер әдепкі бойынша құрастырылады; қосымша мүмкіндіктер қажет емес.

2. **GPU қосылған құрастыру (міндетті емес)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU қолдауы құрастыру кезінде қолжетімді `nvcc` бар SM80+ CUDA құралдар жинағын қажет етеді.【crates/fastpq_prover/Cargo.toml:11】

3. **Өзін-өзі сынау**
   ```bash
   cargo test -p fastpq_prover
   ```
   Қаптама алдында Stage6 жолын растау үшін бұны шығарылым құрастырылымына бір рет іске қосыңыз.

### Металл құралдар тізбегін дайындау (macOS)
1. Құру алдында металл пәрмен жолы құралдарын орнатыңыз: GPU құралдар тізбегін алу үшін `xcode-select --install` (CLI құралдары жоқ болса) және `xcodebuild -downloadComponent MetalToolchain`. Құрастыру сценарийі тікелей `xcrun metal`/`xcrun metallib` шақырады және екілік файлдар жоқ болса, тез істен шығады.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. Құбырды CI алдында тексеру үшін құрастыру сценарийін жергілікті түрде көрсетуге болады:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   Бұл сәтті болған кезде құрастыру `FASTPQ_METAL_LIB=<path>` шығарады; орындау уақыты металлибті анықтауды жүктеу үшін сол мәнді оқиды.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3. Металл құралдар тізбегінсіз айқас құрастыру кезінде `FASTPQ_SKIP_GPU_BUILD=1` орнатыңыз; құрастыру ескертуді басып шығарады және жоспарлаушы процессор жолында қалады.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. Метал қолжетімсіз болса, түйіндер процессорға автоматты түрде оралады (жоқ жақтау, қолдау көрсетілмейтін GPU немесе бос `FASTPQ_METAL_LIB`); құрастыру сценарийі env var файлын тазартады және жоспарлаушы төмендетілген нұсқаны тіркейді.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:65】### Шығарылымды тексеру тізімі (6-кезең)
Төмендегі әрбір элемент аяқталмайынша және тіркелгенше FASTPQ шығару билетін бұғаттаңыз.

1. **Секундтық дәлелдеу көрсеткіштері** — Жаңадан түсірілген `fastpq_metal_bench_*.json` және
   `benchmarks.operations` жазбасын растаңыз, мұнда `operation = "lde"` (және шағылыстырылған
   `report.operations` үлгісі) 20000 жолдық жұмыс жүктемесі (32768 толтырылған) үшін `gpu_mean_ms ≤ 950` есептері
   жолдар). Төбеден тыс түсірулер бақылау парағына қол қою үшін қайталауды қажет етеді.
2. **Қол қойылған манифест** — Іске қосу
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   сондықтан босату билетінде манифест пен оның жеке қолтаңбасы болады
   (`artifacts/fastpq_bench_manifest.sig`). Рецензенттер дайджест/қол жұбын бұрын тексереді
   шығарылымды жылжыту.【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】 матрицалық манифест (құрылған
   `scripts/fastpq/capture_matrix.sh` арқылы) қазірдің өзінде 20к жолды кодтады және
   регрессияны түзету.
3. **Дәлелдер тіркемелері** — JSON металл эталонын, stdout журналын (немесе Құралдар ізін) жүктеңіз.
   CUDA/Metal манифест шығыстары және шығару билетіне бөлінген қолтаңба. Бақылау парағына енгізу
   барлық артефактілерге, сонымен қатар төменгі аудиттерге қол қою үшін пайдаланылатын ашық кілт саусақ ізіне сілтеме жасау керек
   тексеру қадамын қайталай алады.【artifacts/fastpq_benchmarks/README.md:65】### Металды тексеру жұмыс процесі
1. GPU қосылған құрастырудан кейін `FASTPQ_METAL_LIB` нүктелерін `.metallib` (`echo $FASTPQ_METAL_LIB`) арқылы растаңыз, осылайша орындалу уақыты оны анық жүктей алады.【crates/fastpq_prover/build.rs:188】
2. GPU жолақтарын мәжбүрлеп қосу арқылы паритет жиынтығын іске қосыңыз:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. Егер анықтау сәтсіз болса, сервер металл ядроларын қолданады және детерминирленген процессордың қалпына келуін тіркейді.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. Бақылау тақталары үшін эталон үлгісін түсіріңіз:\
   құрастырылған металл кітапханасын табыңыз (`fd -g 'fastpq.metallib' target/release/build | head -n1`),
   оны `FASTPQ_METAL_LIB` арқылы экспорттаңыз және іске қосыңыз
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  Канондық `fastpq-lane-balanced` профилі енді әрбір түсіруді 32 768 жолға (2¹⁵) дейін толтырады, сондықтан JSON `rows` және `padded_rows` екеуін де Metal LDE кідірісімен бірге тасымалдайды; `zero_fill` немесе кезек параметрлері GPU LDE мүмкіндігін AppleM сериялы хосттардағы 950 мс (<1 с) мақсатынан асырса, түсіруді қайта іске қосыңыз. Нәтижедегі JSON/логты басқа шығарылым дәлелдерімен бірге мұрағаттау; түнгі macOS жұмыс процесі бірдей іске қосуды орындайды және оның артефактілерін салыстыру үшін жүктеп салады.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  Сізге тек Poseidon телеметриясы қажет болғанда (мысалы, Құралдар ізін жазу үшін), жоғарыдағы пәрменге `--operation poseidon_hash_columns` қосыңыз; орындық әлі де `FASTPQ_GPU=gpu`-ті құрметтейді, `metal_dispatch_queue.poseidon` шығарады және жаңа `poseidon_profiles` блогын қосады, осылайша шығарылым жинағы Poseidon тығырықтылығын нақты құжаттайды.
  Дәлелдемелер қазір `zero_fill.{bytes,ms,queue_delta}` плюс `kernel_profiles` (әр ядро) қамтиды
  толтыру, болжамды ГБ/с және ұзақтық статистикасы) сондықтан GPU тиімділігін графикасыз көрсетуге болады.
  өңделмеген іздерді және `twiddle_cache` блогын (соққылар/өткізбеу + `before_ms`/`after_ms`) қайта өңдеу
  кэштелген twiddle жүктеп салулардың әрекет ететінін дәлелдейді. `--trace-dir` астындағы сымды қайта іске қосады
  `xcrun xctrace record` және
  уақыт белгісі бар `.trace` файлын JSON жанында сақтайды; сіз әлі де тапсырыс бере аласыз
  `--trace-output` (қосымша `--trace-template` / `--trace-seconds` бар) суретке түсіру кезінде
  реттелетін орын/үлгі. JSON жазбалары `metal_trace_{template,seconds,output}` аудит үшін.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】Әрбір түсіруден кейін `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` іске қосылғаннан кейін жарияланым Grafana тақтасы/ескерту жинағы (`dashboards/grafana/fastpq_acceleration.json`, IVM) үшін негізгі метадеректерді (қазір `metadata.metal_trace` қоса) тасымалдайды. Есеп енді әр операцияға `speedup` нысанын (`speedup.ratio`, `speedup.delta_ms`), орауыш көтергіштері `zero_fill_hotspots` (байттар, кідіріс, алынған ГБ/с және I180010 металл кезегі дельта есептегіштеріне) тегістейді. `benchmarks.kernel_summary`, `twiddle_cache` блогын бүлінбеген күйде сақтайды, жаңа `post_tile_dispatches` блогын/қорытындысын көшіреді, осылайша шолушылар түсіру кезінде жұмыс істеген көп жолақты ядроны дәлелдей алады және енді Poseidon микробенчінің дәлелдерін I10705 тақтасына жинақтайды. скалярға қарсы әдепкі кідіріс шикі есепті қайта талдаусыз. Манифест қақпасы бірдей блокты оқиды және оны өткізіп жіберетін GPU дәлелдемелерін қабылдамайды, бұл операторларды плиткадан кейінгі жолды өткізіп жібергенде немесе түсіруді жаңартуға мәжбүр етеді. дұрыс конфигурацияланбаған.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】src/stpq:732】s/sc.
  Poseidon2 Metal ядросы бірдей тұтқаларды бөліседі: `FASTPQ_METAL_POSEIDON_LANES` (32–256, екінің қуаты) және `FASTPQ_METAL_POSEIDON_BATCH` (әр жолақ үшін 1–32 күй) іске қосу енін және әр жолақ жұмысын қайта құрусыз бекітуге мүмкіндік береді; хост бұл мәндерді әрбір жіберу алдында `PoseidonArgs` арқылы өткізеді. Әдепкі бойынша орындау уақыты `MTLDevice::{is_low_power,is_headless,location}` дискретті графикалық процессорларды VRAM деңгейлі іске қосуларға бейімдеу үшін тексереді (≥48GiB хабарланғанда `256×24`, 32GiB кезінде `256×20`, Prometheus басқа жағдайда төмен қуатта қалады) `256×8` (және ескі 128/64 жолақ бөліктері бір жолақ үшін 8/6 күйге жабысады), сондықтан операторлардың көпшілігі ешқашан env параметрлерін қолмен орнатудың қажеті жоқ.【crates/fastpq_prover/src/metal_config.rs:78】【crates/78/sp.9】【crates/7/sp. `fastpq_metal_bench` енді `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` астында өзін қайта орындайды және екі іске қосу профилін де, сонымен қатар скалярлық жолақпен салыстырғанда өлшенген жылдамдықты жазатын `poseidon_microbench` блогын шығарады, осылайша шығару бумалары жаңа ядроның шын мәнінде кішірейетінін дәлелдей алады және Prometheus қамтиды, `poseidon_pipeline` блогы, осылайша Stage7 дәлелдері жаңа орналасу деңгейлерімен қатар бөліктер тереңдігі/қабаттасу түймелерін түсіреді. Қалыпты іске қосу үшін env параметрін орнатылмаған қалдырыңыз; құрылғы қайта орындауды автоматты түрде басқарады, егер бала түсіру іске қосылмаса, сәтсіздіктерді журналға жазады және `FASTPQ_GPU=gpu` орнатылған кезде бірден шығады, бірақ GPU сервері қол жетімді емес, сондықтан дыбыссыз процессорлық резервтер ешқашан перфке енбейді. артефактілер.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】Қаптама `metal_dispatch_queue.poseidon` дельтасы, ортақ `column_staging` есептегіштері немесе `poseidon_profiles`/`poseidon_microbench` дәлел блоктары жоқ Poseidon түсірулерін қабылдамайды, сондықтан операторлар сәтсіздікке ұшыраған немесе шамадан тыс түсірулерді дәлелдеу үшін кез келген түсіруді жаңартуы керек. speedup.【scripts/fastpq/wrap_benchmark.py:732】 Бақылау тақталары немесе CI дельталары үшін дербес JSON қажет болғанда, `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` іске қосыңыз; көмекші оралған артефактілерді де, өңделмеген `fastpq_metal_bench*.json` түсірілімдерін де қабылдайды, `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` әдепкі/скалярлық уақыттармен, баптау метадеректерімен және жазылған жылдамдықпен шығарады.【scripts/fastpq/export_poseidon_py:1crobench.
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` орындау арқылы іске қосуды аяқтаңыз, осылайша Stage6 шығарылымының бақылау тізімі `<1 s` LDE төбесін күшейтеді және шығарылыммен бірге жеткізілетін қол қойылған манифест/дайджест бумасын шығарады. билет.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. Шығарылым алдында телеметрияны тексеріңіз: Prometheus соңғы нүктесін (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) бұраңыз және `telemetry::fastpq.execution_mode` журналдарында күтпеген `resolved="cpu"` бар-жоғын тексеріңіз жазбалар.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. SRE оқу кітаптары детерминистикалық мәнге сәйкес келуі үшін оны әдейі мәжбүрлеу арқылы (`FASTPQ_GPU=cpu` немесе `zk.fastpq.execution_mode = "cpu"`) процессордың қалпына келтіру жолын құжаттаңыз. мінез-құлық.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. Қосымша баптау: әдепкі бойынша хост қысқа жолдар үшін 16 жолды, орташа үшін 32 және `log_len ≥ 10/14` рет 64/128 жолды таңдайды, `log_len ≥ 18` кезінде 256-ға түседі және ол енді ортақ жад тақтасын бес кезеңде, төрт рет төрт рет I10001010010000000000000000000000000000000000 000 000 0000000000000000000000000000000000000000000000000000000000 және `log_len ≥ 18/20/22` үшін 12/14/16 кезеңдері, жұмысты плиткадан кейінгі ядроға апарар алдында. Осы эвристиканы қайта анықтау үшін жоғарыдағы қадамдарды орындамас бұрын `FASTPQ_METAL_FFT_LANES` (8 және 256 арасында екінің қуаты) және/немесе `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) экспорттаңыз. FFT/IFFT және LDE бағандарының пакеттік өлшемдері шешілген ағындар тобының енінен (≈2048 логикалық ағындар, 32 бағанмен шектелген және енді домен өскен сайын 32→16→8→4→2→1 арқылы төмендейді) алынады, ал LDE жолы әлі де өзінің домен қақпақтарын күшейтеді; `FASTPQ_METAL_FFT_COLUMNS` (1–32) детерминирленген FFT пакет өлшемін бекіту үшін және `FASTPQ_METAL_LDE_COLUMNS` (1–32) параметрін хосттар бойынша биттік салыстыру қажет болғанда LDE диспетчеріне бірдей қайта анықтауды қолдану үшін орнатыңыз. LDE тақтасының тереңдігі FFT эвристикасын да көрсетеді — `log₂ ≥ 18/20/22` бар жолдар кең көбелектерді төсеуден кейінгі ядроға бермес бұрын тек 12/10/8 ортақ жад кезеңдерін орындайды — және `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) арқылы бұл шектеуді қайта анықтауға болады. Орындау уақыты барлық мәндерді Металл ядросының аргтары арқылы өткізеді, қолдау көрсетілмейтін қайта анықтауды қысады және шешілген мәндерді журналға жазады, осылайша эксперименттер металды қайта құрмай-ақ қайталанатын болады; JSON эталоны шешілген баптауды да, LDE статистикасы арқылы түсірілген хосттың нөлдік толтыру бюджетін де (`zero_fill.{bytes,ms,queue_delta}`) көрсетеді, осылайша кезек дельталары әрбір түсірумен тікелей байланыстырылады және енді `column_staging` блогын қосады (топтамалар тегістеледі, flatten_ms, күтулер/тексеруге болады) қос буферлі құбыр арқылы енгізілген. GPU нөлдік толтыру телеметриясын хабарлаудан бас тартқанда, құрылғы енді хост-тараптағы буфер тазартудан детерминирленген уақытты синтездейді және оны `zero_fill` блогына енгізеді, сондықтан дәлелдемелерді ешқашан жіберусіз жібермейді. өріс.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. Дискретті Mac компьютерлерінде көп кезекті жіберу автоматты түрде жүзеге асырылады: `Device::is_low_power()` қатені қайтарғанда немесе Металл құрылғы ұяшық/сыртқы орынды хабарлағанда, хост екі `MTLCommandQueue` іске қосады, жұмыс жүктемесі ≥16 бағанды (және желдеткіш шеңбері бойынша масштабталады) тасымалдағаннан кейін ғана желдеткіштер шығады. ұзақ іздер детерминизмге нұқсан келтірместен екі GPU жолағын да бос ұстайды. Барлық машиналарда қайталанатын түсіру қажет болғанда `FASTPQ_METAL_QUEUE_FANOUT` (1–4 кезек) және `FASTPQ_METAL_COLUMN_THRESHOLD` (желдеткішке дейінгі ең аз жалпы бағандар) саясатты қайта анықтаңыз; паритеттік сынақтар бұл қайта анықтауды мәжбүрлейді, сондықтан көп GPU Mac компьютерлері жабық күйде қалады және шешілген желдеткіш/шегі кезек тереңдігінің жанында тіркеледі. телеметрия.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### Мұрағатқа берілетін дәлелдер
| Артефакт | Түсіру | Ескертпелер |
|----------|---------|-------|
| `.metallib` бумасы | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` және `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`, одан кейін `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` және `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | Metal CLI/құралдар тізбегі орнатылғанын және осы тапсырма үшін детерминирленген кітапхананы жасағанын дәлелдейді.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
| Қоршаған ортаның суреті | `echo $FASTPQ_METAL_LIB` құрастырудан кейін; босату билетімен абсолютті жолды сақтаңыз. | Бос шығу Металл өшірілгенін білдіреді; GPU жолақтары жөнелту артефактісінде қолжетімді болып қалатын құнды құжаттарды жазу.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU паритеті журналы | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` және `backend="metal"` немесе төмендету ескертуі бар үзіндіні мұрағаттаңыз. | Құрылымды алға жылжытпас бұрын ядролардың іске қосылатынын (немесе детерминативті түрде кері түсетінін) көрсетеді.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
| Эталондық нәтиже | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; орап, `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` арқылы қол қойыңыз. | оралған JSON жазбалары `speedup.ratio`, `speedup.delta_ms`, FFT баптау, толтырылған жолдар (32,768), байытылған `zero_fill`/`kernel_profiles`, тегістелген Prometheus, `metal_dispatch_queue.poseidon`/`poseidon_profiles` блоктары (`--operation poseidon_hash_columns` пайдаланылған кезде) және бақылау метадеректері, осылайша GPU LDE орташа мәні ≤950 мс және Poseidon <1с болып қалады; пакетті де, жасалған `.json.asc` қолтаңбасын да шығару билетімен бірге сақтаңыз, осылайша бақылау тақталары мен аудиторлар артефактты қайта іске қоспай-ақ тексере алады. жұмыс жүктемелері.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
| Стендтік манифест | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | GPU артефактілерінің екеуін де растайды, LDE орташа мәні `<1 s` төбесін бұзса, BLAKE3/SHA-256 дайджесттерін жазып, қол қойылған манифест шығарса, шығарылымды тексеру тізімі тексерілмей алға жылжи алмайды. метрика.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA бумасы | SM80 зертханалық хостында `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` іске қосыңыз, JSON файлын `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` ішіне орап/қол қойыңыз (бақылау тақталары дұрыс сыныпты таңдауы үшін `--label device_class=xeon-rtx-sm80` пайдаланыңыз), `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` жолын қосыңыз және сақтаңыз. `.json`/`.asc` манифестті қалпына келтірмес бұрын металл артефактімен жұптаңыз. Тіркелген `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` аудиторлар күтетін нақты пакет пішімін көрсетеді.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80 |xt:10t |
| Телеметриялық дәлелдеу | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` плюс іске қосу кезінде шығарылатын `telemetry::fastpq.execution_mode` журналы. | Трафикті қоспас бұрын Prometheus/OTEL `device_class="<matrix>", backend="metal"` (немесе төмендету журналы) ашылуын растайды.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/74rs: |1| Мәжбүрлі процессорлық бұрғылау | `FASTPQ_GPU=cpu` немесе `zk.fastpq.execution_mode = "cpu"` көмегімен қысқа топтаманы іске қосыңыз және төмендету журналын алыңыз. | Шығарылымның ортасында кері қайтару қажет болған жағдайда SRE жұмыс кітаптарын детерминирленген қалпына келтіру жолымен туралайды.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
| Бақылауды түсіру (міндетті емес) | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` көмегімен паритеттік сынақты қайталаңыз және шығарылған жіберу ізін сақтаңыз. | Сынақтарды қайта іске қоспай-ақ кейінірек профильдеу шолулары үшін толтыру/тарау тобының дәлелдерін сақтайды.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

Көптілді `fastpq_plan.*` файлдары осы бақылау тізіміне сілтеме жасайды, сондықтан кезеңдік және өндіру операторлары бірдей дәлелдер ізімен жүреді.【docs/source/fastpq_plan.md:1】

## Қайталанатын құрылымдар
Қайталанатын Stage6 артефактілерін жасау үшін бекітілген контейнер жұмыс процесін пайдаланыңыз:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

Көмекші сценарий `rust:1.88.0-slim-bookworm` құралдар тізбегі кескінін (және GPU үшін `nvidia/cuda:12.2.2-devel-ubuntu22.04`) құрады, құрастыруды контейнер ішінде іске қосады және `manifest.json`, `sha256s.txt` және құрастырылған екілік файлдарды мақсатты шығысқа жазады каталог.【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

Қоршаған ортаны жою:
- `FASTPQ_RUST_IMAGE`, `FASTPQ_RUST_TOOLCHAIN` – айқын Rust негізін/тегтерін бекітіңіз.
- `FASTPQ_CUDA_IMAGE` – GPU артефактілерін жасау кезінде CUDA негізін ауыстырыңыз.
- `FASTPQ_CONTAINER_RUNTIME` – белгілі бір орындалу уақытын мәжбүрлеу; әдепкі `auto` `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` әрекетін жасайды.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – орындалу уақытын автоматты анықтау үшін үтірмен бөлінген артықшылық тәртібі (әдепкі `docker,podman,nerdctl`).

## Конфигурация жаңартулары
1. TOML ішінде орындау уақытының орындалу режимін орнатыңыз:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   Мән `FastpqExecutionMode` арқылы талданады және іске қосу кезінде серверге жіберіледі.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. Қажет болса, іске қосу кезінде қайта анықтау:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI қайта анықтайды, түйін жүктелмес бұрын шешілген конфигурацияны өзгертеді.【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. Әзірлеушілер экспорттау арқылы конфигурацияларға қол тигізбестен анықтауды уақытша мәжбүрлей алады
   `FASTPQ_GPU={auto,cpu,gpu}` екілік жүйені іске қоспас бұрын; қайта анықтау журналға жазылады және құбыр
   әлі де шешілген режимді көрсетеді.【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## Тексеруді тексеру тізімі
1. **Іске қосу журналдары**
   - `telemetry::fastpq.execution_mode` мақсатынан `FASTPQ execution mode resolved` күтіңіз.
     `requested`, `resolved` және `backend` жапсырмалары.【crates/fastpq_prover/src/backend.rs:208】
   - GPU автоматты түрде анықтау кезінде `fastpq::planner` қосымша журналы соңғы жолды хабарлайды.
   - Металлиб сәтті жүктелген кезде `backend="metal"` металл тіректері; компиляция немесе жүктеу сәтсіз болса, құрастыру сценарийі ескерту шығарады, `FASTPQ_METAL_LIB` өшіреді және жоспарлаушы `GPU acceleration unavailable` жазбасын қалдырмай тұрып жазады. CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover.2. **Prometheus көрсеткіштері**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   Есептегіш `record_fastpq_execution_mode` арқылы көбейтіледі (қазір таңбаланған
   `{device_class,chip_family,gpu_kind}`) түйін өз орындалуын шешкен сайын
   режим.【crates/iroha_telemetry/src/metrics.rs:8887】
   - Металл жабу үшін растаңыз
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     орналастыру бақылау тақталарымен қатар өсулер.【crates/iroha_telemetry/src/metrics.rs:5397】
   - `irohad --features fastpq-gpu` көмегімен құрастырылған macOS түйіндері қосымша ашылады
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     және
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}`, сондықтан Stage7 бақылау тақталары
     жұмыс циклін бақылай алады және Prometheus сызаттарынан кезек күтуге болады.【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **Телеметрия экспорты**
   - OTEL құрастырулары бірдей белгілермен `fastpq.execution_mode_resolutions_total` шығарады; қамтамасыз етіңіз
     бақылау тақталары немесе ескертулер GPU белсенді болуы керек кезде күтпеген `resolved="cpu"` үшін бақылайды.

4. **Ақыл-есі дұрыстығын дәлелдеу/тексеру**
   - `iroha_cli` немесе біріктіру арқауы арқылы шағын топтаманы іске қосыңыз және растауларды растаңыз
     бірдей параметрлермен құрастырылған peer.

## Ақаулықтарды жою
- **Ашулы режим GPU хосттарында CPU болып қалады** — екілік жүйенің орнатылғанын тексеріңіз
  `fastpq_prover/fastpq-gpu`, CUDA кітапханалары жүктеуші жолында және `FASTPQ_GPU` мәжбүрлемейді
  `cpu`.
- **Apple Silicon жүйесінде металл қолжетімсіз** — CLI құралдарының орнатылғанын тексеріңіз (`xcode-select --install`), `xcodebuild -downloadComponent MetalToolchain` қайта іске қосыңыз және құрастырудың бос емес `FASTPQ_METAL_LIB` жолын шығарғанына көз жеткізіңіз; бос немесе жетіспейтін мән дизайн бойынша серверді өшіреді.【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` қателері** — дәлелдеуші де, тексеруші де бірдей канондық каталогты пайдаланатынына көз жеткізіңіз
  `fastpq_isi` шығарған; сәйкес емес бет `Error::UnknownParameter`.【crates/fastpq_prover/src/proof.rs:133】
- **Күтпеген CPU қалпына келтіру** — `cargo tree -p fastpq_prover --features` тексеріп,
  GPU құрастыруларында `fastpq_prover/fastpq-gpu` бар екенін растау; `nvcc`/CUDA кітапханаларының іздеу жолында екенін тексеріңіз.
- **Телеметриялық есептегіш жоқ** — түйіннің `--features telemetry` (әдепкі) арқылы іске қосылғанын тексеріңіз
  және OTEL экспорты (қосылған болса) метрикалық құбырды қамтиды.【crates/iroha_telemetry/src/metrics.rs:8887】

## Қайтару процедурасы
Детерминирленген толтырғыш сервері жойылды. Егер регрессия кері қайтаруды қажет етсе,
бұрын белгілі жақсы шығарылым артефактілерін қайта орналастырыңыз және 6-кезеңді қайта шығарар алдында зерттеңіз
екілік. Өзгерістерді басқару шешімін құжаттаңыз және тек кейін ғана алға жылжытуды қамтамасыз етіңіз
регрессия түсініледі.

3. `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` күтілетінді көрсететініне көз жеткізу үшін телеметрияны бақылаңыз
   толтырғышты орындау.

## Аппараттық база
| Профиль | CPU | GPU | Ескертпелер |
| ------- | --- | --- | ----- |
| Анықтама (6-кезең) | AMD EPYC7B12 (32 ядро), 256GiB жедел жады | NVIDIA A10040GB (CUDA12.2) | 20000 жолдың синтетикалық топтамалары ≤1000 мс аяқталуы керек.【docs/source/fastpq_plan.md:131】 |
| тек CPU | ≥32 физикалық ядролар, AVX2 | – | 20000 жол үшін ~0,9–1,2 с күтіңіз; детерминизм үшін `execution_mode = "cpu"` сақтаңыз. |## Регрессия тестілері
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU хосттарында)
- Қосымша алтын арматураны тексеру:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

Осы бақылау тізімінен кез келген ауытқуларды операциялық жұмыс кітабында құжаттаңыз және `status.md` жаңартыңыз.
тасымалдау терезесі аяқталады.