---
lang: kk
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Есептеу жолағы (SSC-1)

Есептеу жолы детерминирленген HTTP стиліндегі қоңырауларды қабылдайды, оларды Kotodama картасына салады
кіріс нүктелері және есеп айырысу мен басқаруды тексеру үшін есепке алуды/түбіртектерді жазады.
Бұл RFC манифест схемасын, қоңырау/түбіртек конверттерін, құмсалғыш қоршауларын,
және бірінші шығарылым үшін конфигурация әдепкі мәндері.

## Манифест

- Схема: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` `1` күйіне бекітілген; басқа нұсқасы бар манифестер қабылданбайды
  валидация кезінде.
- Әрбір бағыт мәлімдейді:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama кіру нүктесінің атауы)
  - кодектерге рұқсат етілген тізім (`codecs`)
  - TTL/газ/сұраныс/жауап шектері (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - детерминизм/орындау класы (`determinism`, `execution_class`)
  - SoraFS кіріс/модель дескрипторлары (`input_limits`, қосымша `model`)
  - бағалар тобы (`price_family`) + ресурс профилі (`resource_profile`)
  - аутентификация саясаты (`auth`)
- Құм жәшік қоршаулары манифест `sandbox` блогында тұрады және барлығына ортақ.
  маршруттар (режим/кездейсоқтық/сақтау және детерминирленген емес жүйе шақыруын қабылдамау).

Мысалы: `fixtures/compute/manifest_compute_payments.json`.

## Қоңыраулар, сұраулар және түбіртектер

- Схема: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome`
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` канондық сұрау хэшін жасайды (тақырыптар сақталады
  детерминирленген `BTreeMap` жүйесінде және пайдалы жүктеме `payload_hash` ретінде тасымалданады).
- `ComputeCall` аттар кеңістігін/маршрутты, кодекті, TTL/газ/жауап қақпағын түсіреді,
  Ресурс профилі + бағалар тобы, аутентификация (`Public` немесе UAID-байланысты)
  `ComputeAuthn`), детерминизм (`Strict` және `BestEffort`), орындау класы
  кеңестер (CPU/GPU/TEE), жарияланған SoraFS кіріс байттары/бөлшектері, қосымша демеуші
  бюджет және канондық сұрау конверті. Сұраныс хэші үшін пайдаланылады
  қайта ойнатудан қорғау және бағыттау.
- Маршруттар қосымша SoraFS үлгісі сілтемелері мен енгізу шектеулерін ендіруі мүмкін
  (кіріктірілген/бөлек бас әріптер); манифест құм жәшігінің ережелері қақпасы GPU/TEE кеңестері.
- `ComputePriceWeights::charge_units` өлшеу деректерін шотталған есептеуге түрлендіреді
  циклдар мен шығу байттары бойынша төбеге бөлу арқылы бірліктер.
- `ComputeOutcome` есептері `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted` немесе `InternalError` және міндетті түрде жауап хэштерін қамтиды/
  аудитке арналған өлшемдер/кодек.

Мысалдар:
- Қоңырау шалыңыз: `fixtures/compute/call_compute_payments.json`
- Түбіртек: `fixtures/compute/receipt_compute_payments.json`

## Құм жәшік және ресурс профильдері- `ComputeSandboxRules` әдепкі бойынша орындау режимін `IvmOnly` етіп құлыптайды,
  сұрау хэшінен тұқымдардың детерминирленген кездейсоқтығы, SoraFS тек оқуға мүмкіндік береді
  қатынасады және детерминирленген емес жүйелік қоңырауларды қабылдамайды. GPU/TEE кеңестері арқылы жабылады
  Орындауды анықтау үшін `allow_gpu_hints`/`allow_tee_hints`.
- `ComputeResourceBudget` циклдерге, сызықтық жадқа, стекке профильдік қақпақтарды орнатады
  өлшемі, IO бюджеті және шығу, сонымен қатар GPU кеңестері мен WASI-lite көмекшілері үшін ауыстырып-қосқыштар.
- Әдепкі бойынша екі профильді (`cpu-small`, `cpu-balanced`) жібереді.
  `defaults::compute::resource_profiles` детерминирленген кері қайтарулары бар.

## Баға және есеп айырысу бірліктері

- Баға отбасылары (`ComputePriceWeights`) циклдарды және шығыс байттарын есептеуге
  бірлік; әдепкі төлем `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)`
  `unit_label = "cu"`. Отбасылар манифесттерде `price_family` арқылы кілттелген және
  қабылдау кезінде орындалады.
- Өлшеу жазбаларында `charged_units` плюс өңделмеген цикл/кіру/шығу/ұзақтық бар
  салыстыру қорытындылары. Зарядтар орындалу класы бойынша күшейтіледі және
  детерминизм көбейткіштері (`ComputePriceAmplifiers`) және шектелген
  `compute.economics.max_cu_per_call`; шығу арқылы қысылады
  `compute.economics.max_amplification_ratio` байланысты жауапты күшейтуге.
- Демеушілер бюджеттері (`ComputeCall::sponsor_budget_cu`) қарсы орындалады
  бір қоңырауға/күнделікті шектеулер; шот бірлігі жарияланған демеуші бюджеттен аспауы керек.
- Басқару бағасының жаңартулары тәуекелдер класының шегін пайдаланады
  `compute.economics.price_bounds` және жазылған негізгі отбасылар
  `compute.economics.price_family_baseline`; пайдалану
  `ComputeEconomics::apply_price_update` жаңарту алдында дельталарды тексеру үшін
  белсенді отбасы картасы. Torii конфигурация жаңартулары пайдаланылады
  `ConfigUpdate::ComputePricing`, және kiso оны бірдей шектеулермен қолданады
  басқару өңдеулерін детерминистік күйде ұстаңыз.

## Конфигурация

Жаңа есептеу конфигурациясы `crates/iroha_config/src/parameters` ішінде өмір сүреді:

- Пайдаланушы көрінісі: `Compute` (`user.rs`) env қайта анықтауларымен:
  - `COMPUTE_ENABLED` (әдепкі `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Баға/экономика: `compute.economics` түсіреді
  `max_cu_per_call`/`max_amplification_ratio`, төлемді бөлу, демеушілік шектеулер
  (шақыру бойынша және күнделікті КО), бағалар отбасының негізгі көрсеткіштері + тәуекел сыныптары/шектері
  басқару жаңартулары және орындау класының мультипликаторлары (GPU/TEE/best-efort).
- Нақты/әдепкілер: `actual.rs` / `defaults.rs::compute` талданған экспозиция
  `Compute` параметрлері (аттар кеңістігі, профильдер, баға топтары, құм жәшік).
- Жарамсыз конфигурациялар (бос аттар кеңістігі, әдепкі профиль/отбасы жоқ, TTL қақпағы
  инверсиялар) талдау кезінде `InvalidComputeConfig` түрінде көрсетіледі.

## Сынақтар мен құрылғылар

- Детерминистикалық көмекшілер (`request_hash`, баға белгілеу) және арматура бойынша айналу сапарлары
  `crates/iroha_data_model/src/compute/mod.rs` (қараңыз: `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- JSON құрылғылары `fixtures/compute/` жүйесінде жұмыс істейді және деректер үлгісімен орындалады
  регрессияны қамтуға арналған сынақтар.

## SLO жабдықтары мен бюджеттері- `compute.slo.*` конфигурациясы шлюз SLO тұтқаларын көрсетеді (ұшу кезегі)
  тереңдік, RPS шегі және кідіріс мақсаттары) ішінде
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Әдепкі: 32
  ұшу кезінде, әр бағытта 512 кезекте, 200 RPS, p50 25ms, p95 75ms, p99 120ms.
- SLO қорытындыларын және сұрауды/шығуды түсіру үшін жеңіл орындық арқанды іске қосыңыз
  сурет: `cargo run -p xtask --bin compute_gateway -- орындық [манифест_жолы]
  [итерациялар] [бірлесу] [шығатын_дир]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 итерация, параллельдік 16, шығыстар
  `artifacts/compute_gateway/bench_summary.{json,md}`). Орындық пайдаланады
  детерминирленген пайдалы жүктемелер (`fixtures/compute/payload_compute_payments.json`) және
  жаттығу кезінде қайталама соқтығысуды болдырмау үшін сұрау бойынша тақырыптар
  `echo`/`uppercase`/`sha3` кіру нүктелері.

## SDK/CLI паритеті

- Канондық құрылғылар `fixtures/compute/` астында жұмыс істейді: манифест, қоңырау, пайдалы жүктеме және
  шлюз стиліндегі жауап/түбіртек орналасуы. Пайдалы жүктеме хэштері қоңырауға сәйкес болуы керек
  `request.payload_hash`; көмекші пайдалы жүк тұрады
  `fixtures/compute/payload_compute_payments.json`.
- CLI `iroha compute simulate` және `iroha compute invoke` жібереді:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` тұрады
  `javascript/iroha_js/src/compute.js` астында регрессия сынақтары бар
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` бірдей құрылғыларды жүктейді, пайдалы жүктеме хэштерін тексереді,
  және кіру нүктелерін сынақтармен имитациялайды
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift көмекшілерінің барлығы бірдей Norito құрылғыларын бөліседі, осылайша SDK файлдары
  a түймесін баспай сұрау салуды және хэшті офлайн режимінде өңдеуді растаңыз
  жұмыс істейтін шлюз.