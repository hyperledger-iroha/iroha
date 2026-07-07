---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3852a0f039b664344f9cbce7d2514172cfe97cd838b68755f764d4fe183b22cc
source_last_modified: "2026-01-05T09:28:11.898207+00:00"
translation_last_reviewed: 2026-02-07
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
:::

SF-3 Iroha/Torii процесін SoraFS сақтау провайдеріне айналдыратын бірінші іске қосылатын `sorafs-node` қорабын жеткізеді. Бұл жоспарды [түйінді сақтау нұсқаулығы](node-storage.md), [провайдерді қабылдау саясаты](provider-admission-policy.md) және [сақтау сыйымдылығы нарығының жол картасы](storage-capacity-marketplace.md) жеткізілімдерін реттілік кезінде пайдаланыңыз.

## Мақсатты ауқым (М1 кезең)

1. **Бөлім қоймасын біріктіру.** `sorafs_car::ChunkStore` файлын конфигурацияланған деректер каталогында блок байттарын, манифесттерді және PoR ағаштарын сақтайтын тұрақты сервермен ораңыз.
2. **Шлюздің соңғы нүктелері.** Norito HTTP соңғы нүктелерін Torii процесінде түйреуішті жіберу, бөлікті алу, PoR үлгісін алу және сақтау телеметриясы үшін ашыңыз.
3. **Конфигурациялық сантехника.** `SoraFsStorage` конфигурация құрылымын (қосылған жалауша, сыйымдылық, каталогтар, параллельдік шектеулері) `iroha_config`, `iroha_core` және I18NI0000003X арқылы қосыңыз.
4. **Квота/жоспарлау.** Оператор анықтайтын диск/параллельдік шектеулер мен кері қысыммен кезек сұрауларын орындау.
5. **Телеметрия.** Іске қосу сәттілігі, бөлікті алу кідірісі, сыйымдылықты пайдалану және PoR үлгісін таңдау нәтижелері үшін көрсеткіштерді/журналдарды шығарыңыз.

## Жұмыстың бұзылуы

### A. Қорап және модуль құрылымы

| Тапсырма | Ие(лер) | Ескертпелер |
|------|----------|-------|
| `crates/sorafs_node` модульдерімен жасаңыз: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Сақтау тобы | Torii интеграциясы үшін қайта пайдалануға болатын түрлерін қайта экспорттау. |
| `SoraFsStorage` арқылы салыстырылған `StorageConfig` іске қосыңыз (пайдаланушы → нақты → әдепкі). | Storage Team / Config WG | Norito/`iroha_config` қабаттарының детерминирленген күйінде қалуына көз жеткізіңіз. |
| `NodeHandle` қасбетін қамтамасыз етіңіз Torii түйреуіштерді/алуларды жіберу үшін пайдаланады. | Сақтау тобы | Сақтаудың ішкі бөліктерін және асинхронды сантехниканы инкапсуляциялаңыз. |

### B. Тұрақты бөлшектер дүкені

| Тапсырма | Ие(лер) | Ескертпелер |
|------|----------|-------|
| Дискідегі манифест индексі (`sled`/`sqlite`) бар `sorafs_car::ChunkStore` дискінің орамасын жасаңыз. | Сақтау тобы | Детерминистік орналасу: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` көмегімен PoR метадеректерін (64KiB/4KiB ағаштары) сақтаңыз. | Сақтау тобы | Қайта іске қосқаннан кейін қайталауды қолдау; Сыбайлас жемқорлыққа тез төтеп беру. |
| Іске қосу кезінде тұтастықты қайталауды жүзеге асыру (қайта өңдеу манифесттері, аяқталмаған түйреуіштерді кесу). | Сақтау тобы | Torii блогын қайта ойнату аяқталғанша бастаңыз. |

### C. Шлюздің соңғы нүктелері

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `GET /v1/sorafs/pin`, `POST /v1/sorafs/pin/register`, `GET /v1/sorafs/pin/{digest_hex}` | Read the pin registry, register paid manifest pins, and fetch bounded manifest pin details. | Validate chunker profiles, manifest payloads, pin policy, fee receipt context, aliases, and successor links before queueing the signed transaction. |
| `POST /v1/sorafs/storage/pin`, `POST /v1/sorafs/storage/fetch`, `POST /v1/sorafs/storage/token` | Store payload bytes for an approved manifest, fetch content ranges, and issue storage access tokens. | Enforce quotas, token policy, provider capability checks, and scheduler/back-pressure limits. |
| `GET /v1/sorafs/storage/manifest/{manifest_id}`, `GET /v1/sorafs/storage/plan/{manifest_id}`, `GET /v1/sorafs/storage/car/{manifest_id}`, `GET /v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}` | Serve bounded manifest metadata, deterministic chunk plans, CAR bytes, and individual chunk bytes. | Keep readback arrays bounded while preserving total counts and verify digest/path bindings before streaming bytes. |
| `GET /v1/sorafs/storage/peers`, `GET /v1/sorafs/storage/state`, `POST /v1/sorafs/storage/por-sample`, `POST /v1/sorafs/storage/por-challenge`, `POST /v1/sorafs/storage/por-proof`, `POST /v1/sorafs/storage/por-verdict` | Report peer/storage state and exercise local PoR sampling, challenge, proof, and verdict plumbing. | Reuse chunk-store sampling, update telemetry, and preserve governance-verdict replay state. |


Жұмыс уақыты `sorafs_node::por` арқылы PoR өзара әрекеттесетін сантехника: трекер әрбір `PorChallengeV1`, `PorProofV1` және `AuditVerdictV1` жазады, осылайша `CapacityMeter` көрсеткіштері өзгермейді. Torii логикасы.【crates/sorafs_node/src/scheduler.rs#L147】

Іске асыру туралы ескертулер:

- `norito::json` пайдалы жүктемелері бар Torii Axum стекін пайдаланыңыз.
- Жауаптар үшін Norito схемаларын қосыңыз (`PinResultV1`, `FetchErrorV1`, телеметриялық құрылымдар).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` енді артта қалу тереңдігін, сонымен қатар ең ескі дәуірді/мерзімді және
  әр провайдер үшін ең соңғы сәтті/сәтсіздік уақыт белгілерін қуаттайды
  `sorafs_node::NodeHandle::por_ingestion_status` және Torii жазады
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` өлшеуіштері бақылау тақталары.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:18 83】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Жоспарлаушы және квотаны орындау

| Тапсырма | Мәліметтер |
|------|---------|
| Диск квотасы | Дискідегі байттарды қадағалау; `max_capacity_bytes` мәнінен асқан кезде жаңа түйреуіштерді қабылдамаңыз. Болашақ саясаттар үшін шығару ілмектерін қамтамасыз етіңіз. |
| Сәйкестікті алу | Ғаламдық семафор (`max_parallel_fetches`) және SF-2d ауқымының шектерінен алынған провайдер бюджеттері. |
| Pin кезегі | Көрнекті қабылдау жұмыстарын шектеңіз; кезек тереңдігі үшін Norito күйдің соңғы нүктелерін көрсетіңіз. |
| PoR каденциясы | `por_sample_interval_secs` басқаратын фондық жұмысшы. |

### E. Телеметрия және журнал жүргізу

Көрсеткіштер (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (`result` белгілері бар гистограмма)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Журналдар / оқиғалар:

- Басқаруды енгізуге арналған құрылымдық Norito телеметриясы (`StorageTelemetryV1`).
- Қолдану >90% немесе PoR ақаулық жолағы шекті мәннен асқанда ескертеді.

### F. Тестілеу стратегиясы

1. **Бірлік сынақтары.** Бөлшек қоймасының тұрақтылығы, квота есептеулері, жоспарлаушы инварианттары (`crates/sorafs_node/src/scheduler.rs` қараңыз).  
2. **Интеграциялық сынақтар** (`crates/sorafs_node/tests`). PIN → кері сапарды алу, қалпына келтіруді қайта іске қосу, квотаны қабылдамау, PoR сынамасын растауды тексеру.  
3. **Torii біріктіру сынақтары.** Torii жады қосылған күйде іске қосыңыз, HTTP соңғы нүктелерін `assert_cmd` арқылы орындаңыз.  
4. **Хаос жол картасы.** Болашақ жаттығулар дискінің таусылуын, баяу IO-ны, провайдерді жоюды модельдейді.

## Тәуелділіктер

- SF-2b қабылдау саясаты — түйіндердің жарнамадан бұрын қабылдау конверттерін тексеруін қамтамасыз етіңіз.  
- SF-2c сыйымдылық нарығы — телеметрияны сыйымдылық декларацияларына қайта қосыңыз.  
- SF-2d жарнама кеңейтімдері — қол жетімді болған кезде ауқым мүмкіндігін + ағындық бюджеттерді пайдаланады.

## Белгілі кезеңнен шығу критерийлері

- `cargo run -p sorafs_node --example pin_fetch` жергілікті құрылғыларға қарсы жұмыс істейді.  
- Torii exposes the current `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` route surface and passes integration tests.
- Құжаттама ([түйінді сақтау нұсқаулығы](node-storage.md)) конфигурация әдепкілері + CLI мысалдарымен жаңартылды; оператордың жұмыс кітабы қол жетімді.  
- Бақылау тақталарында көрінетін телеметрия; сыйымдылықтың қанықтығы және PoR ақаулары үшін конфигурацияланған ескертулер.

## Құжаттама және операциялық жеткізілім

- [Түйінді сақтау анықтамасын](node-storage.md) конфигурация әдепкілерімен, CLI пайдалануымен және ақаулықтарды жою қадамдарымен жаңартыңыз.  
- SF-3 дамып келе жатқанда [түйін операцияларының жұмыс кітабын](node-operations.md) іске асыруға сәйкестендіріңіз.  
- Keep API reference for `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` endpoints aligned with the OpenAPI manifest.