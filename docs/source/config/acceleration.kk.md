---
lang: kk
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Acceleration & Norito эвристикалық анықтама

`[accel]` блогы `iroha_config` арқылы өтеді
`crates/irohad/src/main.rs:1895` `ivm::set_acceleration_config` ішіне. Әрбір хост
VM құру алдында бірдей тұтқаларды қолданады, осылайша операторлар анықтауға болады
скаляр/SIMD резервтік нұсқаларын қол жетімді етіп сақтай отырып, қандай GPU серверлеріне рұқсат етілгенін таңдаңыз.
Swift, Android және Python байланыстары бірдей манифестті көпір қабаты арқылы жүктейді
осы әдепкі мәндерді құжаттау WP6-C аппараттық жеделдету артындағы блоктан шығарады.

### `accel` (аппараттық жеделдету)

Төмендегі кестеде `docs/source/references/peer.template.toml` және
`iroha_config::parameters::user::Acceleration` анықтамасы, қоршаған ортаны ашу
әрбір кілтті қайта анықтайтын айнымалы.

| Негізгі | Env var | Әдепкі | Сипаттама |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` | SIMD/NEON/AVX орындауын қосады. `false` кезде, VM детерминирленген паритет түсіруді жеңілдету үшін векторлық амалдар мен Merkle хэшингіне арналған скалярлық серверлерді мәжбүрлейді. |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` | CUDA сервері компиляцияланғанда және орындалу уақыты барлық алтын векторлық тексерулерден өткенде қосады. |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` | MacOS құрастыруларында металл серверін қосады. Тіпті шын болса да, теңдік сәйкессіздіктері орын алса, металдың өзін-өзі сынаулары орындалу уақытында серверді өшіре алады. |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0` (автоматты) | Орындау уақыты қанша физикалық GPU іске қосатынын көрсетеді. `0` «сәйкес аппараттық желдеткіш» дегенді білдіреді және `GpuManager` арқылы қысылады. |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Merkle жапырағы хэштеу GPU-ға түсіруден бұрын ең аз жапырақтар қажет. Осы шекті мәннен төмен мәндер PCIe (`crates/ivm/src/byte_merkle_tree.rs:49`) жүктемесін болдырмау үшін процессорда хэштеуді жалғастырады. |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0` (жаһандық мұралау) | GPU шегі үшін металға тән қайта анықтау. `0` кезде, металл `merkle_min_leaves_gpu` мұралайды. |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0` (жаһандық мұралау) | GPU шегі үшін CUDA-арнайы қайта анықтау. `0` кезде, CUDA `merkle_min_leaves_gpu` мұралайды. |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0` (ішкі 32768) | ARMv8 SHA-2 нұсқаулары GPU хэшингінде жеңетін ағаш өлшемін көрсетеді. `0` `32_768` жапырақтарының құрастырылған әдепкі мәнін сақтайды (`crates/ivm/src/byte_merkle_tree.rs:59`). |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0` (ішкі 32768) | Жоғарыдағыдай, бірақ SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) пайдаланатын x86/x86_64 хосттары үшін. |

`enable_simd` сонымен қатар RS16 өшіру кодтауын басқарады (Torii DA қабылдау + құрал). Оны өшіріңіз
шығыстарды аппараттық құрал бойынша детерминирленген күйде сақтай отырып, скалярлық паритеттерді генерациялауды күшейту.

Мысал конфигурация:

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

Соңғы бес кілт үшін нөлдік мәндер «құрастырылған әдепкі мәнді сақтау» дегенді білдіреді. Хосттар болмауы керек
қайшылықты қайта анықтауды орнату (мысалы, тек CUDA шектерін мәжбүрлеу кезінде CUDA өшіру),
әйтпесе сұрау еленбейді және сервер жаһандық саясатты ұстануды жалғастырады.

### Орындалу уақыты күйін тексеруҚолданбаны суретке түсіру үшін `cargo xtask acceleration-state [--format table|json]` іске қосыңыз
металл/CUDA жұмыс уақытының денсаулық биттерімен қатар конфигурация. Пәрменді тартады
ағымдағы `ivm::acceleration_config`, паритет күйі және жабысқақ қате жолдары (егер
сервер өшірілген) сондықтан операциялар нәтижені тікелей паритетке бере алады
бақылау тақталары немесе оқиғаларға шолулар.

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
```

Суретті автоматтандыру (JSON) арқылы қабылдау қажет болғанда `--format json` пайдаланыңыз.
кестеде көрсетілген өрістерді қамтиды).

`acceleration_runtime_errors()` енді SIMD неліктен скалярға оралғанын айтады:
`disabled by config`, `forced scalar override`, `simd unsupported on hardware`, немесе
Анықтау сәтті болған кезде `simd unavailable at runtime`, бірақ орындау әлі де орындалады
векторсыз. Қайта анықтауды өшіру немесе саясатты қайта қосу хабарды тастайды
SIMD қолдайтын хосттарда.

### Паритет тексерулері

Детерминирленген нәтижелерді дәлелдеу үшін `AccelerationConfig` параметрін тек CPU және жеделдету арасында аударыңыз.
`poseidon_instructions_match_across_acceleration_configs` регрессиясы іске қосады
Poseidon2/6 операциялық кодтары екі рет — алдымен `enable_cuda`/`enable_metal` `false` мәніне орнатылған, содан кейін
екеуі қосулы—және GPU бар кезде бірдей шығыстарды және CUDA теңдігін бекітеді.【crates/ivm/tests/crypto.rs:100】
Кері ұшы бар-жоғын жазу үшін `acceleration_runtime_status()` файлын іске қосумен бірге түсіріңіз
конфигурацияланған/зертханалық журналдарда қолжетімді.

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

### GPU әдепкі мәндері және эвристика

`MerkleTree` GPU жүктеуі `8192` кезінде басталады, ал CPU SHA-2
артықшылық шегі әр архитектура үшін `32_768` жапырақтарында қалады. Бірде CUDA, не
Металл бар немесе денсаулықты тексеру арқылы өшірілген болса, VM автоматты түрде түседі
SIMD/скаляр хэшингке оралу және жоғарыдағы сандар детерминизмге әсер етпейді.

`max_gpus` `GpuManager` ішіне берілген бассейн өлшемін қысады. `max_gpus = 1` параметрі қосулы
көп GPU хосттары жеделдетуге мүмкіндік бере отырып, телеметрияны қарапайым етеді. Операторлар жасай алады
FASTPQ немесе CUDA Poseidon тапсырмалары үшін қалған құрылғыларды сақтау үшін осы қосқышты пайдаланыңыз.

### Келесі жеделдету мақсаттары мен бюджеттері

Соңғы FastPQ металл ізі (`fastpq_metal_bench_20k_latest.json`, 32K жол × 16
бағандар, 5 итер) ZK жұмыс жүктемелеріне басымдық беретін Poseidon бағанының хэшингін көрсетеді:

- `poseidon_hash_columns`: CPU орташа **3,64s** және GPU орташа **3,55s** (1,03×).
- `lde`: CPU орташа **1,75s** және GPU орташа **1,57s** (1,12×).

IVM/Crypto келесі жеделдетуде осы екі ядроға бағытталған. Базалық бюджеттер:

- Скаляр/SIMD тепе-теңдігін жоғарыдағы орталық процессорлық құралда немесе одан төмен ұстаңыз және түсіріңіз
  `acceleration_runtime_status()` әр жүгірумен қатар, металл/CUDA қолжетімді болуы мүмкін
  бюджеттік нөмірлермен тіркеледі.
- `poseidon_hash_columns` үшін мақсатты ≥1,3× жылдамдықты арттыру және `lde` үшін ≥1,2× рет реттелген металл
  және CUDA ядролары шығыстарды немесе телеметриялық белгілерді өзгертпей жерге түседі.

JSON ізін және `cargo xtask acceleration-state --format json` суретін тіркеңіз
болашақ зертхана жұмыс істейді, сондықтан CI/регрессиялар бюджетті де, сервердің денсаулығын да растай алады
тек процессорды және жеделдетілген жұмыстарды салыстыру.

### Norito эвристика (компиляция уақытының әдепкі мәндері)Norito орналасу және қысу эвристикасы `crates/norito/src/core/heuristics.rs` ішінде өмір сүреді
және әрбір екілік жүйеге жинақталады. Олар орындау уақытында конфигурацияланбайды, бірақ ашық
кірістер SDK және оператор топтарына GPU бір рет Norito қалай әрекет ететінін болжауға көмектеседі
қысу ядролары қосылған.
Жұмыс кеңістігі қазір әдепкі бойынша қосылған `gpu-compression` мүмкіндігімен Norito құрастырады,
сондықтан GPU zstd серверлері құрастырылады; орындау уақытының қолжетімділігі әлі де аппараттық құралға байланысты,
көмекші кітапхана (`libgpuzstd_*`/`gpuzstd_cuda.dll`) және `allow_gpu_compression`
конфигурациялау жалауы. `cargo build -p gpuzstd_metal --release` және көмегімен Металл көмекшісін жасаңыз
жүктеуші жолына `libgpuzstd_metal.dylib` қойыңыз. Ағымдағы металл көмекшісі GPU жұмыс істейді
сәйкестікті табу/дәйектілік генерациялау және жәшік ішіндегі детерминирленген zstd кадрын пайдаланады
хосттағы кодтаушы (Huffman/FSE + жақтау жинағы); декодтау жәшік ішіндегі жақтауды пайдаланады
GPU блогының декодтауы сым қосылғанша қолдау көрсетілмейтін кадрлар үшін CPU zstd резерві бар декодер.

| Өріс | Әдепкі | Мақсаты |
|-------|---------|---------|
| `min_compress_bytes_cpu` | `256` байт | Бұдан төмен, пайдалы жүктемелер үстеме шығындарды болдырмау үшін zstd толығымен өткізіп жібереді. |
| `min_compress_bytes_gpu` | `1_048_576` байт (1МиБ) | `norito::core::hw::has_gpu_compression()` шын болғанда, осы шектегі немесе одан жоғары пайдалы жүктемелер GPU zstd параметріне ауысады. |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` | Сәйкесінше <32KiB және ≥32KiB пайдалы жүктемелер үшін CPU қысу деңгейлері. |
| `zstd_level_gpu` | `1` | Командалық кезектерді толтыру кезінде кідіріс уақытын тұрақты сақтау үшін консервативті GPU деңгейі. |
| `large_threshold` | `32_768` байт | «кіші» және «үлкен» CPU zstd деңгейлері арасындағы өлшем шекарасы. |
| `aos_ncb_small_n` | `64` жолдары | Осы жолдың астындағы адаптивті кодтаушылар ең аз пайдалы жүктемені таңдау үшін AoS және NCB орналасуларын тексереді. |
| `combo_no_delta_small_n_if_empty` | `2` жолдары | 1–2 жолдарда бос ұяшықтар болған кезде u32/id дельта кодтауларын қосуды болдырмайды. |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` | Дельталар кем дегенде екі қатар болған кезде ғана енеді. |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` | Барлық дельта түрлендірулері әдепкі бойынша дұрыс енгізілген енгізулер үшін қосылады. |
| `combo_enable_name_dict` | `true` | Ұқсату коэффициенттері жадтың үстеме шығынын ақтаған кезде әр бағандағы сөздіктерге рұқсат береді. |
| `combo_dict_ratio_max` | `0.40` | Жолдардың 40%-дан астамы бөлек болған кезде сөздіктерді өшіріңіз. |
| `combo_dict_avg_len_min` | `8.0` | Сөздіктерді құру алдында жолдың орташа ұзындығы ≥8 болуын талап етіңіз (қысқа бүркеншік аттар желіде қалады). |
| `combo_dict_max_entries` | `1024` | Жадты шектеулі пайдалануды қамтамасыз ету үшін сөздік жазбаларындағы қатты қақпақ. |

Бұл эвристика GPU қолдайтын хосттарды тек процессорға арналған құрдастармен теңестіреді: селектор
ешқашан сым пішімін өзгертетін шешім қабылдамайды және шекті мәндер бекітіледі
шығарылымға. Профильдеу жақсы шығынсыздық нүктелерін ашқанда, Norito жаңартады
канондық `Heuristics::canonical` енгізу және `docs/source/benchmarks.md` плюс
`status.md` өзгерісті нұсқаланған дәлелдермен бірге жазыңыз.GPU zstd көмекшісі тіпті егер
тікелей шақырылады (мысалы, `norito::core::gpu_zstd::encode_all` арқылы), соншалықты кішкентай
пайдалы жүктемелер GPU қолжетімділігіне қарамастан әрқашан CPU жолында қалады.

### Ақауларды жою және паритеттерді тексеру тізімі

- `cargo xtask acceleration-state --format json` көмегімен суреттің орындалу уақыты күйі және сақтаңыз
  кез келген сәтсіз журналдармен бірге шығыс; есеп конфигурацияланған/қол жетімді серверлерді көрсетеді
  плюс паритет/соңғы қате жолдары.
- Дрейфті болдырмау үшін жеделдету паритетінің регрессиясын жергілікті түрде қайта іске қосыңыз:
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  (тек процессорды іске қосады, содан кейін жылдамдатады). Іске қосу үшін `acceleration_runtime_status()` жазыңыз.
- Егер сервер өздігінен сынақтан өтпесе, түйінді тек CPU режимінде желіде ұстаңыз (`enable_metal =
  false`, `enable_cuda = false`) және түсірілген паритет шығысымен оқиғаны ашыңыз
  серверді қосудың орнына. Нәтижелер режимдерде детерминистік болып қалуы керек.
- **CUDA паритеттік түтін (лабораториялық NV аппараттық құралы):** Іске қосу
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  sm_8x аппараттық құралында `cargo xtask acceleration-state --format json` түсіріп, тіркеңіз
  эталондық артефактілерге күй суреті (GPU үлгісі/драйвер кіреді).