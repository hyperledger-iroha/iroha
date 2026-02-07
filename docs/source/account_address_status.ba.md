---
lang: ba
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Иҫәп яҙмаһы адресы үтәү статусы (ADDR-2)

Статус: 2026-03-30-сы ҡабул ителгән.  
Хужалар: Мәғлүмәттәр моделе командаһы / QA гильдия  
Юл картаһы һылтанмаһы: ADDR-2 — ике форматтағы үтәлеш люкс

### 1. Обзор

- Фикстура: `fixtures/account/address_vectors.json` (IH58 (өҫтөнлөк) + ҡыҫылған (`sora`, икенсе-иң яҡшы) + күп сиг ыңғай/тиҫкәре осраҡтар).
- Скоп: детерминистик V1 файҙалы йөктәр ҡаплаған йәшерен-подлючение, урындағы-12, Глобаль реестр, һәм мультисиг контроллерҙары менән тулы хата таксономияһы.
- Таратыу: Rust мәғлүмәттәре-моделе буйынса бүленгән, Torii, JS/TS, Свифт һәм Android SDKs; CI уңышһыҙлыҡҡа осрай, әгәр ниндәй ҙә булһа ҡулланыусы тайпыла.
- Хәҡиҡәт сығанағы: генератор `crates/iroha_data_model/src/account/address/compliance_vectors.rs`-та йәшәй һәм `cargo xtask address-vectors` аша фашлана.
### 2. Регенерация һәм раҫлау

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Флагтар:

- `--out <path>` — махсус өйөмдәр етештергәндә теләк буйынса өҫтөнлөк (`fixtures/account/address_vectors.json` тиклем ғәҙәттәгесә).
- `--stdout` — JSON дискҡа яҙыу урынына stdout-ҡа сығарырға.
- `--verify` — ағымдағы файлды яңы генерацияланған йөкмәткегә ҡаршы сағыштырырға (дрейфта тиҙ; `--stdout` менән ҡулланырға мөмкин түгел).

### 3. Артефакт матрицаһы

| Ер өҫтө | үтәү | Иҫкәрмәләр |
|--------|-------------|-------|
| Кире мәғлүмәт-модель | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON-ды анализлай, канонлы файҙалы йөктәрҙе реконструкциялай, IH58 (өҫтөнлөклө)/ҡыҫылған (`sora`, икенсе-иң яҡшы)/канон конверсия + структуралы хаталарҙы тикшерә. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Сервер яғында кодектарҙы раҫлай, шуға күрә Torii X58 (өҫтөнлөк)/ҡыҫылған (`sora`, икенсе иң яҡшы) детерминистик яҡтан дөрөҫ формалаштырылған IH58-ҙән баш тарта. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Көҙгөләр V1 ҡоролмалары (IH58 өҫтөнлөк бирә/ҡыҫылған (`sora`) икенсе-иң яҡшы/тулывидность) һәм раҫлай Norito-стиль хата кодтары өсөн һәр кире осраҡта. |
| Свифт СДК | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | IH58 (өҫтөнлөк)/ҡыҫылған (`sora`, икенсе иң яҡшы) декодлау, мультисиг файҙалы йөктәр һәм Apple платформаларында хаталарҙы өҫтөн ҡуйыу күнекмәләре. |
| Андроид СДК | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Котлин/Java бәйләүҙәре канонлы ҡоролма менән тура килтереп ҡалыуын тәьмин итә. |

### 4. Мониторинг һәм күренекле эш- Статус отчет: был документ `status.md` һәм юл картаһынан бәйләнгән, шуға күрә аҙна һайын үткәрелгән отзывтар һаулыҡ һаҡлау һаулығын раҫлай ала.
- порталы резюмеһын төҙөү: ҡарағыҙ **Һылтанма → Иҫәп яҙмаһы адресы үтәү ** документ порталында (`docs/portal/docs/reference/account-address-status.md`) тышҡы йөҙө синопсисы өсөн.
- Prometheus һәм приборҙар таҡталары: ҡасан ғына һеҙ раҫлайһығыҙ SDK күсермәһе, ярҙамсыһы менән идара итеү `--metrics-out` (һәм теләк буйынса `--metrics-label`) шулай Prometheus текст файлы `account_address_fixture_check_status{target=…}` in in ingest. Grafana приборҙар таҡтаһы **Иҫәп яҙмаһы Адресы Фикстура Статус** (`dashboards/grafana/account_address_fixture_status.json`) үткәреү/уңышһыҙлыҡ һандары өҫтөндәге/уңышһыҙлыҡтар һаны өҫтөндә канонлы SHA-256 аудит дәлилдәре өсөн дисциплина. Иҫкәртергә ҡасан ниндәй ҙә булһа маҡсатлы хәбәр итеү `0`.
- Torii метрикаһы: `torii_address_domain_total{endpoint,domain_kind}` хәҙер һәр уңышлы анализланған иҫәп туранан-тура, көҙгө `torii_address_invalid_total`/Prometheus. Ҡайһы бер `domain_kind="local12"` етештереүҙә иҫкәртмә һәм счетчиктарҙы көҙгө SRE `address_ingest` приборҙар таҡтаһына, шуға күрә урындағы-12 пенсия ҡапҡаһы аудит дәлилдәре бар.
- Fixtur ярҙамсыһы: Prometheus скачиваний йәки канонлы JSON тикшерергә шулай SDK автоматлаштырыу релиз ала ала/тикшерергә өйөм ҡул күсермәһе/йәбештереү, шул уҡ ваҡытта опциональ яҙыу Prometheus метрика. Миҫал:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  Ярҙам `account_address_fixture_check_status{target="android"} 1` ҡасан маҡсатлы матчтар, өҫтәүенә `account_address_fixture_remote_info` / `account_address_fixture_local_info` датчиктар, улар SHA-256 дисциплиналарын фашлай. Миңле файлдар отчеты `account_address_fixture_local_missing`.
  Автоматлаштырыу урау: шылтыратыу `ci/account_fixture_metrics.sh` cron/CI-нан консолидацияланған текст файлын сығарыу өсөн (Prometheus стандарт ғәҙәттәгесә). Үткәргес ҡабатланған `--target label=path` яҙмалары (факультатив рәүештә `::https://mirror/...` ҡушымтаны өҫтөнлөклө итеп ҡуйығыҙ) шулай Prometheus бер файлды ҡаплай, һәр SDK/CLI күсермәһен ҡаплай. GitHub эш ағымы `address-vectors-verify.yml` был ярҙамсы инде канон ҡоролмаһына ҡаршы идара итә һәм SRE ашау өсөн `account-address-fixture-metrics` артефактын тейәй.