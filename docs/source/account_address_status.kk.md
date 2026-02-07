---
lang: kk
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Тіркелгі мекенжайының сәйкестік күйі (ADDR-2)

Күйі: Қабылданды 30.03.2026 ж  
Иелері: Data Model Team / QA Guild  
Жол картасының анықтамасы: ADDR-2 — Қос форматты сәйкестік жиынтығы

### 1. Шолу

- Бекіту: `fixtures/account/address_vectors.json` (IH58 (қалау) + қысылған (`sora`, екінші ең жақсы) + мультисиг оң/теріс жағдайлар).
- Қолдану аясы: жасырын-әдепкі, Жергілікті-12, Жаһандық тізілім және толық қате таксономиясы бар мультисиг контроллерлерін қамтитын детерминирленген V1 пайдалы жүктемелері.
- Тарату: Rust деректер үлгісі, Torii, JS/TS, Swift және Android SDK арқылы ортақ; Кез келген тұтынушы ауытқыса, CI сәтсіз аяқталады.
- Ақиқат көзі: генератор `crates/iroha_data_model/src/account/address/compliance_vectors.rs` ішінде тұрады және `cargo xtask address-vectors` арқылы ашылады.
### 2. Регенерация және тексеру

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Жалаулар:

- `--out <path>` — арнайы бумаларды жасау кезіндегі қосымша қайта анықтау (әдепкі бойынша `fixtures/account/address_vectors.json`).
- `--stdout` — дискіге жазудың орнына stdout файлына JSON шығарады.
- `--verify` — ағымдағы файлды жаңадан жасалған мазмұнмен салыстыру (дрейфте жылдам орындалмайды; `--stdout` көмегімен пайдалану мүмкін емес).

### 3. Артефакт матрицасы

| Беттік | Орындау | Ескертпелер |
|---------|-------------|-------|
| Rust деректер үлгісі | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON талдайды, канондық пайдалы жүктемелерді қайта құрастырады және IH58 (таңдаулы)/қысылған (`sora`, екінші ең жақсы)/канондық түрлендірулер + құрылымдық қателерді тексереді. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Torii дұрыс емес IH58 (таңдаулы)/қысылған (`sora`, екінші ең жақсы) пайдалы жүктемелерден анық бас тартуы үшін серверлік кодектерді тексереді. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Mirrors V1 құрылғылары (IH58 қолайлы/сығымдалған (`sora`) екінші ең жақсы/толық ен) және әрбір теріс жағдай үшін Norito стиліндегі қате кодтарын бекітеді. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Apple платформаларында IH58 (қалаулы)/қысылған (`sora`, екінші ең жақсы) декодтау, мультисиг пайдалы жүктемелері және қателерді жою жаттығулары. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Котлин/Java байламдарының канондық арматураға сәйкес келуін қамтамасыз етеді. |

### 4. Мониторинг және үздік жұмыс- Күй туралы есеп беру: бұл құжат `status.md` және жол картасы арқылы байланыстырылған, сондықтан апта сайынғы шолулар арматураның денсаулығын тексере алады.
- Әзірлеуші ​​порталының қысқаша мазмұны: сыртқы конспект үшін құжаттар порталындағы (`docs/portal/docs/reference/account-address-status.md`) **Анықтама → Тіркелгі мекенжайының сәйкестігі** бөлімін қараңыз.
- Prometheus және бақылау тақталары: SDK көшірмесін тексерген сайын, `--metrics-out` (және міндетті түрде `--metrics-label`) көмегімен көмекшіні іске қосыңыз, осылайша Prometheus мәтіндік файл жинағышы I1000000. Grafana бақылау тақтасы **Тіркелгі мекенжайы бекіту күйі** (`dashboards/grafana/account_address_fixture_status.json`) әр бетке өту/сәтсіздік сандарын береді және аудиторлық дәлелдер үшін канондық SHA-256 дайджестін көрсетеді. Кез келген мақсат `0` хабарлағанда ескерту.
- Torii көрсеткіштері: `torii_address_domain_total{endpoint,domain_kind}` енді `torii_address_invalid_total`/`torii_address_local8_total` шағылысатын әрбір сәтті талданған тіркелгі литералы үшін шығарады. Өндірістегі кез келген `domain_kind="local12"` трафигі туралы ескерту және есептегіштерді SRE `address_ingest` бақылау тақтасына қайталаңыз, осылайша Local-12 зейнеткерлік қақпасында тексерілетін дәлелдер болады.
- Бекіту көмекшісі: `scripts/account_fixture_helper.py` канондық JSON жүктеп алады немесе тексереді, осылайша SDK шығарылымының автоматтандырылуы Prometheus метрикасын жазу кезінде қолмен көшіру/қоюсыз пакетті алуға/тексеруге мүмкіндік береді. Мысалы:

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

  Көмекші мақсат сәйкес келгенде `account_address_fixture_check_status{target="android"} 1` жазады, сонымен қатар SHA-256 дайджесттерін көрсететін `account_address_fixture_remote_info` / `account_address_fixture_local_info` өлшегіштері. Жетіспейтін файлдар `account_address_fixture_local_missing` есебі.
  Автоматтандыру орамы: біріктірілген мәтіндік файлды шығару үшін cron/CI жүйесінен `ci/account_fixture_metrics.sh` нөміріне қоңырау шалыңыз (әдепкі `artifacts/account_fixture/address_fixture.prom`). Қайталанатын `--target label=path` жазбаларын өткізіңіз (міндетті түрде көзді қайта анықтау үшін мақсатқа `::https://mirror/...` қосыңыз), осылайша Prometheus әрбір SDK/CLI көшірмесін қамтитын бір файлды қырып тастайды. GitHub жұмыс процесі `address-vectors-verify.yml` бұл көмекшіні канондық қондырғыға қарсы іске қосып қойған және SRE қабылдау үшін `account-address-fixture-metrics` артефакті жүктеп салады.