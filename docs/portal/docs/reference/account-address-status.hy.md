---
lang: hy
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b92bdfc323a4bc031ca7f2237f238d5d515f7238791a6ec9c50b55e361c85560
source_last_modified: "2026-01-28T17:11:30.639071+00:00"
translation_last_reviewed: 2026-02-07
id: account-address-status
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
---

Կանոնական ADDR-2 փաթեթը (`fixtures/account/address_vectors.json`) գրավում է
IH58 (նախընտրելի), սեղմված (`sora`, երկրորդ լավագույնը; կես/ամբողջական լայնությունը), բազմանշանակ և բացասական հարմարանքներ:
Յուրաքանչյուր SDK + Torii մակերեսը հիմնված է նույն JSON-ի վրա, որպեսզի մենք կարողանանք հայտնաբերել ցանկացած կոդեկ
շեղվել մինչև արտադրության վրա հասնելը: Այս էջը արտացոլում է ներքին կարգավիճակի համառոտագիրը
(`docs/source/account_address_status.md` արմատային պահոցում) այնքան պորտալ
Ընթերցողները կարող են հղում կատարել աշխատանքի ընթացքին՝ առանց մոնո-ռեպո փորփրելու:

## Վերականգնեք կամ հաստատեք փաթեթը

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Դրոշներ:

- `--stdout` — թողարկեք JSON-ը` ժամանակավոր ստուգման համար stdout-ի համար:
- `--out <path>` — գրել այլ ուղու վրա (օրինակ՝ տեղական փոփոխությունների փոփոխման ժամանակ):
- `--verify` — համեմատել աշխատանքային պատճենը նոր ստեղծված բովանդակության հետ (չի կարող
  համակցված լինի `--stdout`-ի հետ):

CI աշխատանքային հոսքը **Address Vector Drift** աշխատում է `cargo xtask address-vectors --verify`
ցանկացած ժամանակ, երբ սարքը, գեներատորը կամ փաստաթղթերը փոխվում են, որպեսզի անմիջապես ծանուցեն վերանայողներին:

## Ո՞վ է սպառում հարմարանքը:

| Մակերեւութային | Վավերացում |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (սերվեր) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Յուրաքանչյուր զրահ է պտտվում կանոնական բայթ + IH58 + սեղմված (`sora`, երկրորդ լավագույն) կոդավորումները և
ստուգում է, որ Norito ոճի սխալի կոդերը համընկնում են բացասական դեպքերի համար նախատեսված սարքի հետ:

## Ավտոմատացման կարիք կա՞:

Release tooling-ը կարող է սկրիպտը թարմացնել օգնականի հետ
`scripts/account_fixture_helper.py`, որը առբերում կամ հաստատում է կանոնականը
փաթեթ առանց պատճենելու/տեղադրելու քայլերի.

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Օգնականն ընդունում է `--source` կամ `IROHA_ACCOUNT_FIXTURE_URL`
շրջակա միջավայրի փոփոխական, այնպես որ SDK CI աշխատանքները կարող են մատնանշել իրենց նախընտրած հայելին:
Երբ `--metrics-out` մատակարարվում է, օգնականը գրում է
`account_address_fixture_check_status{target=\"…\"}` կանոնականի հետ միասին
SHA-256 digest (`account_address_fixture_remote_info`) ուստի Prometheus տեքստային ֆայլ
կոլեկտորները և Grafana վահանակը `account_address_fixture_status` կարող են ապացուցել
յուրաքանչյուր մակերես մնում է համաժամանակյա: Զգուշացեք, երբ թիրախը հայտնում է `0`: Համար
բազմաբնույթ մակերևույթի ավտոմատացում, օգտագործեք փաթաթան `ci/account_fixture_metrics.sh`
(ընդունում է կրկնվող `--target label=path[::source]`), ուստի հերթապահ թիմերը կարող են հրապարակել
մեկ համախմբված `.prom` ֆայլ՝ հանգույց արտահանող տեքստային ֆայլեր հավաքողի համար:

## Պե՞տք է ամբողջական համառոտագիր:

ADDR-2-ի համապատասխանության ամբողջական կարգավիճակը (սեփականատերեր, մոնիտորինգի պլան, բաց գործողությունների կետեր)
ապրում է `docs/source/account_address_status.md`-ում՝ պահեստի երկայնքով
Հասցեի կառուցվածքի RFC-ով (`docs/account_structure.md`): Օգտագործեք այս էջը որպես a
արագ գործառնական հիշեցում; հետաձգել ռեպո փաստաթղթերը՝ խորը առաջնորդության համար: