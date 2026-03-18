---
lang: hy
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Հաշվի հասցեի համապատասխանության կարգավիճակ (ADDR-2)

Կարգավիճակ՝ Ընդունված 2026-03-30  
Սեփականատերեր՝ Data Model Team / QA Guild  
Ճանապարհային քարտեզի տեղեկանք՝ ADDR-2 — Երկու ձևաչափի համապատասխանության փաթեթ

### 1. Ընդհանուր ակնարկ

- Սարքավորում՝ `fixtures/account/address_vectors.json` (I105 (նախընտրելի) + սեղմված (`sora`, երկրորդ լավագույն) + բազմանշանակ դրական/բացասական դեպքեր):
- Շրջանակ. դետերմինիստական ​​V1 օգտակար բեռներ, որոնք ընդգրկում են անուղղակի լռելյայն, Local-12, Համաշխարհային գրանցամատյան և բազմանշանակ կարգավորիչներ՝ ամբողջական սխալների տաքսոնոմիայով:
- Բաշխում. համօգտագործվում է Rust տվյալների մոդելի, Torii, JS/TS, Swift և Android SDK-ների միջև; CI-ն ձախողվում է, եթե որևէ սպառող շեղվի:
- Ճշմարտության աղբյուր. գեներատորն ապրում է `crates/iroha_data_model/src/account/address/compliance_vectors.rs`-ում և բացահայտվում է `cargo xtask address-vectors`-ի միջոցով:
### 2. Վերականգնում և ստուգում

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Դրոշներ:

- `--out <path>` — կամընտիր փոխարինում ժամանակավոր փաթեթներ արտադրելիս (կանխադրված է `fixtures/account/address_vectors.json`):
- `--stdout` — թողարկեք JSON-ը stdout-ի համար՝ սկավառակի վրա գրելու փոխարեն:
- `--verify` — համեմատեք ընթացիկ ֆայլը նոր ստեղծված բովանդակության հետ (արագ ձախողվում է դրեյֆի ժամանակ, չի կարող օգտագործվել `--stdout`-ի հետ):

### 3. Արտեֆակտ մատրիցա

| Մակերեւութային | Կատարման | Ծանոթագրություններ |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | Վերլուծում է JSON-ը, վերակառուցում է կանոնական բեռները և ստուգում I105 (նախընտրելի)/սեղմված (`sora`, երկրորդ լավագույն)/կանոնական փոխարկումները + կառուցվածքային սխալները: |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Վավերացնում է սերվերի կողմի կոդեկները, որպեսզի Torii-ը վճռականորեն հրաժարվի սխալ ձևավորված I105 (նախընտրելի)/սեղմված (`sora`, երկրորդ լավագույն) օգտակար բեռներից: |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Հայելիներ V1 սարքերը (I105 նախընտրելի/սեղմված (`sora`) երկրորդ լավագույն/ամբողջ լայնությունը) և հաստատում է Norito ոճի սխալի կոդերը յուրաքանչյուր բացասական դեպքի համար: |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Զորավարժություններ I105 (նախընտրելի)/սեղմված (`sora`, երկրորդ լավագույն) վերծանման, բազմակողմանի բեռների և Apple հարթակներում երևացող սխալների վրա: |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Ապահովում է, որ Kotlin/Java կապերը համահունչ մնան կանոնական սարքի հետ: |

### 4. Մոնիտորինգ և գերազանց աշխատանք- Կարգավիճակի հաշվետվություն. այս փաստաթուղթը կապված է `status.md`-ից և ճանապարհային քարտեզից, որպեսզի շաբաթական վերանայումները կարողանան ստուգել սարքի առողջությունը:
- Մշակողների պորտալի ամփոփագիրը. տես **Հղում → Հաշվի հասցեի համապատասխանությունը** փաստաթղթերի պորտալում (`docs/portal/docs/reference/account-address-status.md`) արտաքին տեսք ունեցող ամփոփագրի համար:
- Prometheus և վահանակներ. ամեն անգամ, երբ դուք հաստատում եք SDK-ի պատճենը, գործարկեք օգնականը `--metrics-out`-ով (և ըստ ցանկության՝ `--metrics-label`), որպեսզի Prometheus տեքստային ֆայլերի հավաքիչը կարողանա կլանել I1803NI000: Grafana վահանակ **Հաշվի հասցեի տեղադրման կարգավիճակը** (`dashboards/grafana/account_address_fixture_status.json`) ներկայացնում է անցումների/խափանման հաշվարկները յուրաքանչյուր մակերեսի համար և ներկայացնում է կանոնական SHA-256 բովանդակությունը աուդիտորական ապացույցների համար: Զգուշացում, երբ որևէ թիրախ հաղորդում է `0`:
- Torii չափումներ. `torii_address_domain_total{endpoint,domain_kind}` այժմ թողարկվում է յուրաքանչյուր հաջողությամբ վերլուծված հաշվի բառացի՝ արտացոլելով `torii_address_invalid_total`/`torii_address_local8_total`: Զգուշացրեք արտադրության ցանկացած `domain_kind="local12"` երթևեկության մասին և արտացոլեք հաշվիչները SRE `address_ingest` վահանակի վրա, որպեսզի Local-12 կենսաթոշակային դարպասն ունենա աուդիտի ենթակա ապացույցներ:
- Սարքավորումների օգնական. `scripts/account_fixture_helper.py`-ը ներբեռնում է կամ ստուգում է կանոնական JSON-ը, որպեսզի SDK-ի թողարկման ավտոմատացումը կարողանա վերցնել/ստուգել փաթեթը առանց ձեռքով պատճենելու/տեղադրելու՝ ցանկության դեպքում գրելով Prometheus չափումները: Օրինակ՝

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

  Օգնականը գրում է `account_address_fixture_check_status{target="android"} 1`, երբ թիրախը համընկնում է, գումարած `account_address_fixture_remote_info` / `account_address_fixture_local_info` չափիչները, որոնք բացահայտում են SHA-256 մարսողությունը: Բացակայող ֆայլերի հաշվետվություն `account_address_fixture_local_missing`:
  Ավտոմատացման փաթաթան. զանգահարեք `ci/account_fixture_metrics.sh` cron/CI-ից՝ համախմբված տեքստային ֆայլ թողարկելու համար (կանխադրված `artifacts/account_fixture/address_fixture.prom`): Անցեք `--target label=path` կրկնվող գրառումները (ըստ ցանկության կցեք `::https://mirror/...` յուրաքանչյուր թիրախի՝ աղբյուրը վերացնելու համար), այնպես որ Prometheus-ը քերծում է մեկ ֆայլ՝ ծածկելով յուրաքանչյուր SDK/CLI պատճենը: GitHub աշխատանքային հոսքը `address-vectors-verify.yml` արդեն գործարկում է այս օգնականը կանոնական սարքի դեմ և վերբեռնում է `account-address-fixture-metrics` արտեֆակտը՝ SRE-ի ներթափանցման համար: