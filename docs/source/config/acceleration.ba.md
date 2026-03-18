---
lang: ba
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Тиҙләтеү һәм Norito Эвристика һылтанмаһы

`[accel]` блок `iroha_config` ептәре аша .
`crates/irohad/src/main.rs:1895` `ivm::set_acceleration_config`-ҡа. Һәр алып барыусы
VM-ды экземплярь алдынан шул уҡ ручкаларҙы ҡуллана, шуға күрә операторҙар детерминистик рәүештә .
ҡабул итеү, ниндәй GPU бекэндтар рөхсәт ителә, шул уҡ ваҡытта һаҡлау скаляр/SIMD fallbacks доступный.
Свифт, Android һәм Python бәйләүҙәре күпер ҡатламы аша бер үк манифест йөкләй, шулай уҡ
был ғәҙәттәгесә документлаштырыу WP6-C блокировканы асыу аппарат-тиҙләтеү артта ҡалған.

### `accel` (аппарат тиҙләнеше)

Түбәндәге таблицала `docs/source/references/peer.template.toml` һәм
`iroha_config::parameters::user::Acceleration` билдәләмәһе, тирә-яҡ мөхитте фашлау
үҙгәртеүсән, ул һәр асҡысты өҫтөн ҡуя.

| Асҡыс | Env var | Ғәҙәттәгесә | Тасуирлама |
|----|---------|----------|------------||
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX башҡарыуын индереү. Ҡасан `false`, виртуаль көстәр скаляр бэкэндтар өсөн вектор опс һәм Меркл хеширование еңеләйтеү өсөн детерминистик паритет тотоу. |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | CUDA бекэнд рөхсәт ҡасан ул төҙөлә һәм йөрөү ваҡыты бөтә алтын-вектор тикшерелгән үтә. |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | Металл бэкэнд macOS төҙөүҙәр өҫтөндә эшләй. Хатта ҡасан дөрөҫ, металл үҙ-үҙеңде һынауҙар һаман да өҙөп була бэкэнд йүгерә ваҡытында, әгәр паритет тап килмәүҙәре осрай. |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (авто) | Ҡапҡалар нисә физик GPUs эшләү ваҡыты инициализацияһы. `0` тигәнде аңлата “матч аппарат фан-аут” һәм `GpuManager` менән ҡыҫтырылған. |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Минималь япраҡтар кәрәк, әлегә Меркл япраҡлы хеширование йөкләүҙәр GPU. Ҡиммәттәр аҫтында был сикте һаҡлау хеширование процессорҙа ҡотолоу өсөн PCIe накладной (`crates/ivm/src/byte_merkle_tree.rs:49`). |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (ҡаралған глобаль) | Металл-специфик өҫтөнлөк өсөн GPU сиге. Ҡасан `0`, Металл мираҫы `merkle_min_leaves_gpu`. |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (ҡаршылыҡлы глобаль) | CUDA-специфик өҫтөнлөк өсөн GPU сиге. Ҡасан `0`, CUDA мираҫы `merkle_min_leaves_gpu`. |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (32768 эске) | Ҡапҡас ағас ҙурлығы, унда ARMv8 SHA-2 инструкциялары еңергә тейеш GPU хешинг. `0` `32_768` япраҡтары (`crates/ivm/src/byte_merkle_tree.rs:59`) компиляцияланған ғәҙәттәгесә һаҡлай. |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | Norito (32768 эске) | Шул уҡ өҫтәге кеүек, әммә x86/x86_64 хост өсөн SHA-NI ҡулланып (`crates/ivm/src/byte_merkle_tree.rs:63`). |

`enable_simd` шулай уҡ RS16 юйыу кодлауын контролдә тота (Torii DA ингест + инструменттар). Уны өҙөү өсөн .
көс скаляр паритет генерацияһы, шул уҡ ваҡытта аппарат буйынса детерминистик сығыштарҙы һаҡлау.

Миҫал конфигурацияһы:

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

Һуңғы биш асҡыс өсөн нуль ҡиммәттәре “компиляцияланған ғәҙәттән тыш хәлде һаҡлау” тигәнде аңлата. Хужалар тейеш түгел
ҡапма-ҡаршылыҡлы өҫтөнлөктәр ҡуйыла (мәҫәлән, CUDA-тик сиктәр мәжбүр иткәндә CUDA инвалидлыҡ итә, ә CUDA,
юғиһә үтенес иғтибарға алынмай һәм бэкэнд глобаль сәйәсәтте үтәүен дауам итә.

### Йөрөү ваҡыты буйынса дәүләтЙүгерергә `cargo xtask acceleration-state [--format table|json]` снимок өсөн ғариза
конфигурация менән бер рәттән Металл/CUDA эшләү ваҡыты һаулыҡ биттары. Команда тарта
Хәҙерге `ivm::acceleration_config`, паритет статусы, һәм йәбешкәк хата телмәрҙәре (әгәр а
бекэнд инвалид булды) шуға күрә операциялар туранан-тура паритетҡа һөҙөмтәне туҡландыра ала
приборҙар панелдәре йәки инцидент тикшерелгән.

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
``` X

Ҡулланыу `--format json` ҡасан снимок кәрәк автоматлаштырыу аша ингестировать (JSON
таблицала күрһәтелгән шул уҡ ҡырҙар бар).

`acceleration_runtime_errors()` хәҙер SIMD ни өсөн скалярға кире төшкәнен саҡыра:
`disabled by config`, `forced scalar override`, `simd unsupported on hardware`, йәки
`simd unavailable at runtime` асыҡланғанда уңышҡа өлгәшкәндә, әммә башҡарыу һаман да эшләй
векторҙарһыҙ. Ҡабул итеү йәки яңынан мөмкинлек бирә сәйәсәте тураһында хәбәрҙе төшөрә
SIMD-ны яҡлаған алып барыусыларҙа.

### Паритет чектары

Flip `AccelerationConfig` процессор араһында-тик һәм тиҙ-өҫтөндә детерминистик һөҙөмтәләрҙе иҫбатлау өсөн.
`poseidon_instructions_match_across_acceleration_configs` регрессия йүгерә
Poseidon2/6 опкодтар ике тапҡыр — беренсе тапҡыр `enable_cuda`/`enable_metal` X-ға `false` тиклем ҡуйылған, һуңынан
100】
Каптыр `acceleration_runtime_status()` йүгерә менән бер рәттән, фондтармы, юҡмы икәнлеген теркәү өсөн
лаборатория журналдарында конфигурацияланған/доступный.

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### GPU ғәҙәттәгесә & эвристика

Norito GPU offload of foods `8192` ҡалдыра ғәҙәттәгесә, һәм процессор SHA-2.
өҫтөнлөк сиктәре `32_768` япраҡтарында архитектурала ҡала. Ҡасан да CUDA ла, ни.
Металл бар йәки һаулыҡ һаҡлау тикшерелгән өҙөлгән, ВМ автоматик рәүештә төшә .
кире SIMD/скаляр хеширование һәм өҫтәге һандар детерминизмға йоғонто яһамай.

`max_gpus` зажимдар бассейн күләме `GpuManager` XX. `max_gpus = 1` 1990 йылда ҡуйыу.
күп GPU хужалары телеметрия ябай тота, шул уҡ ваҡытта тиҙләтеү мөмкинлеген бирә. Операторҙар ала
был коммутатор FASTPQ йәки CUDA Poseidon эштәре өсөн ҡалған ҡоролмаларын һаҡлау өсөн ҡулланыу.

### Киләһе тиҙләтеү маҡсаттары & бюджеттар

Һуңғы FastPQ металл эҙ (`fastpq_metal_bench_20k_latest.json`, 32K рәттәр × 16
бағана, 5 iters) күрһәтә Посейдон бағана хеширование өҫтөнлөк ZK эш йөкләмәһе:

- `poseidon_hash_columns`: процессор тигәнде аңлата **3.64s** ҡаршы GPU тигәнде аңлата **3.55s** (1.03×).
- `lde`: процессор тигәнде аңлата **1.75s** ҡаршы GPU тигәнде аңлата **1.57s** (1.12×).

IVM/Крипто был ике ядроға сираттағы аккалет һыпыртыуҙа маҡсатлы буласаҡ. База бюджеттары:

- скаляр һаҡлау/SIMD паритетында йәки аҫтында процессор өҫтә тигәнде аңлата, һәм тотоу
  `acceleration_runtime_status()` һәр йүгерә менән бергә шулай Металл/CUDA доступность
  бюджет һандары менән логин.
- ≥1,3× тиҙлекте `poseidon_hash_columns` өсөн маҡсатлы һәм ≥1,2× `lde` өсөн бер тапҡыр Metal Metal
  һәм CUDA ядролары ер, етештереү йәки телеметрия ярлыҡтары үҙгәрмәйенсә.

Беркетергә JSON эҙ һәм `cargo xtask acceleration-state --format json` снимок .
киләсәк лаборатория эшләй, шулай CI/регрессиялар раҫлай ала, бюджеттар һәм бэкэнд һаулыҡ, шул уҡ ваҡытта .
сағыштырыу процессоры-тик ҡаршы.

### Norito эвристикаһы (компиляция-ваҡыт ғәҙәттәгесә)Norito’s макеты һәм ҡыҫыу эвристикаһы йәшәй `crates/norito/src/core/heuristics.rs`
һәм һәр бинарға төҙөлә. Улар йөрөү ваҡытында конфигурациялана, әммә фашлау
индереүҙәр SDK һәм оператор командалары ярҙам итә, нисек Norito үҙен бер тапҡыр тотасаҡ, тип күҙаллай GPU
ҡыҫыу ядролары эшләй.
Эш урыны хәҙер Norito төҙөй, `gpu-compression` функцияһы менән ғәҙәттәгесә эшләй,
тимәк, GPU zstd бэкэндтар төҙөлә; эшләү ваҡыты һаман да аппаратҡа бәйле,
ярҙамсы китапханаһы (`libgpuzstd_*`/`gpuzstd_cuda.dll`), һәм `allow_gpu_compression` XX
конфиг флагы. `cargo build -p gpuzstd_metal --release` һәм 1990 йылдарҙа Металл ярҙамсыһын төҙөү һәм
урыны `libgpuzstd_metal.dylib` тейәгес юлында. Хәҙерге Металл ярҙамсыһы ГПУ-ны етәкләй
матч-табыу/эҙмә-эҙлеклелек генерациялау һәм знстд кадрын ҡуллана һәм детерминистик zstd .
кодер (Хаффман/ФСЭ + кадр йыйыу) хужала; декод йәшниктәге кадрҙы ҡуллана
декодер менән процессор zstd fallback өсөн ярҙам ителмәгән рамкалар тиклем GPU блок декод сымлы.

| Ялан | Ғәҙәттәгесә | Маҡсат |
|-------|---------|----------|
| `min_compress_bytes_cpu` | `256` байт | Был түбәндә, файҙалы йөкләмәләр skip zstd тулыһынса ҡотолоу өсөн накладной. |
| `min_compress_bytes_gpu` | Norito байт (1МиБ) | Түләүҙәр йәки өҫтөндә был сиккә күсеү GPU zstd ҡасан `norito::core::hw::has_gpu_compression()` дөрөҫ. |
| `zstd_level_small` / `zstd_level_large` | ```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
``` X / `3` | Процессор ҡыҫыу кимәле өсөн <32KiB һәм ≥32KiB файҙалы йөкләмәләр ярашлы. |
| `zstd_level_gpu` | `1` | Консерватив GPU кимәлендә латентлыҡ эҙмә-эҙлекле һаҡлау өсөн, шул уҡ ваҡытта команда сираттарын тултырыу. |
| `large_threshold` | `32_768` байттар | Ҙурлыҡ сиге араһында “бәләкәй” һәм “ҙур” процессор zstd кимәлдәре. |
| `aos_ncb_small_n` | `64` рәттәре | Был рәт адаптив кодерҙары аҫтында зонд һәм AoS һәм NCB макеттары иң бәләкәй файҙалы йөк йыйыу өсөн. |
| `combo_no_delta_small_n_if_empty` | `2` рәттәре | 1–2 рәт булғанда буш күҙәнәктәр булғанда u32/ид дельта кодлауын мөмкинлек бирә. |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Дельтас бер тапҡыр ғына тибеп, кәмендә ике рәт бар. |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | Бөтә дельта трансформациялары ла тәртипле индереүҙәр өсөн ғәҙәттәгесә эшләй. |
| `combo_enable_name_dict` | `true` | Хит нисбәттәре хәтер накладнойын аҡлағанда, бағанаға һүҙлектәр рөхсәт итә. |
| `combo_dict_ratio_max` | `0.40` | 40%-тан ашыу рәт айырым булғанда һүҙлектәр өҙөлгән. |
| `combo_dict_avg_len_min` | `8.0` | Уртаса еп оҙонлоғо ≥8 һүҙлектәр төҙөү алдынан талап итә (ҡыҫҡа псевдонимдар рәттә ҡала). |
| `combo_dict_max_entries` | `1024` | Ҡаты ҡапҡас һүҙлек яҙмалары буйынса гарантиялау өсөн сикләнгән хәтер ҡулланыу. |

Был эвристика GPU-мөмкинлекле хосттарҙы процессор менән тура килтереп, тик тиҫтерҙәре: селектор .
бер ҡасан да ҡарар ҡабул итеү, тип үҙгәртер ине сым форматында, һәм сиктәре нығытылған
бер релиз. Ҡасан профилләштереү яҡшыраҡ өҙөлгән мәрәй асып, Norito яңыртыу .
канон `Heuristics::canonical` тормошҡа ашырыу һәм `docs/source/benchmarks.md` плюс
`status.md` версияланған дәлилдәр менән бер рәттән үҙгәреште теркәй.GPU zstd ярҙамсыһы шул уҡ `min_compress_bytes_gpu` өҙөклөктө үтәй, хатта ҡасан да булһа.
туранан-тура шылтырата (мәҫәлән, `norito::core::gpu_zstd::encode_all` аша), шул тиклем бәләкәй
файҙалы йөктәр һәр ваҡыт процессор юлында ҡала, ҡарамаҫтан, GPU доступность.

### Проблема һәм паритет тикшерелгән исемлеге

- `cargo xtask acceleration-state --format json` менән Snapshot эшләү ваҡыты дәүләт
  теләһә ниндәй етешһеҙлектәр журналдар менән бер рәттән сығыш; отчет күрһәтә конфигурацияланған/доступный бэкэндтар
  плюс паритет/һуңғы хаталы ҡылдар.
- Ҡабаттан йүгерергә локаль паритет регрессияһы локаль дрейф инҡар итеү өсөн:
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (процессор йүгерә-тик шул саҡта accel-on өҫтөндә). Яҙма `acceleration_runtime_status()` өсөн йүгерергә.
- Әгәр ҙә бэкэнд үҙ-үҙеңде тикшергәндән мәхрүм ителһә, төйөнде процессорҙы процессорҙа ғына һаҡларға (ҡуйылған_металл = enable_metal .
  fland`, `enable_cuda = ялған`) һәм әсирлеккә алынған паритет сығышы менән инцидент асыу
  урынына көсләп бэкэнд өҫтөндә. Һөҙөмтәләр режимдар буйынса детерминистик булып ҡалырға тейеш.
- **CUDA паритет төтөнө (лаборатория НВ аппарат):** Йүгереп йөрөү
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x аппарат, тотоу `cargo xtask acceleration-state --format json`, һәм беркетергә
  статус снимок (GPU моделе/водитель индерелгән) эталон артефакттар.