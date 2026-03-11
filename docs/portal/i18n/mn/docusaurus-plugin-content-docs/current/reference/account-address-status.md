---
id: account-address-status
lang: mn
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Каноник ADDR-2 багц (`fixtures/account/address_vectors.json`) зураг авдаг
I105 (давуу), шахсан (`sora`, хоёрдугаарт; хагас/бүрэн өргөн), олон гарын үсэг, сөрөг бэхэлгээ.
SDK + Torii гадаргуу бүр ижил JSON дээр тулгуурладаг тул бид ямар ч кодлогчийг илрүүлэх боломжтой
drift before it hits production. This page mirrors the internal status brief
(`docs/source/account_address_status.md` эх репозитор) тиймээс портал
Уншигчид моно репо ухахгүйгээр ажлын урсгалыг лавлаж болно.

## Багцыг дахин үүсгэх эсвэл баталгаажуулах

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Тугнууд:

- `--stdout` — түр шалгалтад зориулж JSON-г stdout руу ялгаруулна.
- `--out <path>` — өөр зам руу бичих (жишээ нь, орон нутгийн өөрчлөлтийг ялгах үед).
- `--verify` - ажлын хуулбарыг шинээр үүсгэсэн контенттой харьцуулах (боломжгүй
  be combined with `--stdout`).

CI ажлын урсгал **Address Vector Drift** нь `cargo xtask address-vectors --verify`-г ажиллуулдаг.
ямар ч үед бэхэлгээ, генератор эсвэл баримт бичиг өөрчлөгдөхөд хянагчдад нэн даруй мэдэгдэнэ.

## Тоног төхөөрөмжийг хэн хэрэглэдэг вэ?

| Гадаргуу | Баталгаажуулалт |
|---------|------------|
| Rust өгөгдлийн загвар | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Тус бүр нь хоёр талдаа каноник байт + I105 + шахсан (`sora`, хоёрдугаарт) кодчилол ба
Norito загварын алдааны кодууд нь сөрөг тохиолдлуудад тохирох эсэхийг шалгадаг.

## Автоматжуулалт хэрэгтэй байна уу?

Суллах хэрэгсэл нь туслагчийн тусламжтайгаар бэхэлгээний шинэчлэлтийг скрипт болгож чадна
`scripts/account_fixture_helper.py`, энэ нь каноникийг татах эсвэл баталгаажуулдаг
Хуулах/буулгах алхамгүйгээр багцлах:

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

Туслагч нь `--source` эсвэл `IROHA_ACCOUNT_FIXTURE_URL`-г хүчингүй болгодог
орчны хувьсагч учраас SDK CI ажлууд нь өөрсдийн сонгосон толин тусгал руугаа чиглүүлэх боломжтой.
`--metrics-out` нийлүүлэх үед туслагч бичнэ
`account_address_fixture_check_status{target=\"…\"}` каноникийн хамт
SHA-256 дижест (`account_address_fixture_remote_info`) тиймээс Prometheus текст файл
цуглуулагчид болон Grafana хяналтын самбар `account_address_fixture_status` нотолж чадна
гадаргуу бүр синхрончлолтой хэвээр байна. Зорилтот `0` гэж мэдээлэх бүрд сэрэмжлүүл. Учир нь
олон гадаргуугийн автоматжуулалт нь `ci/account_fixture_metrics.sh` боодол ашиглана
(давтан `--target label=path[::source]` хүлээн авдаг) тул дуудлагын багууд нийтлэх боломжтой
зангилаа экспортлогч текст файл цуглуулагчийн нэг нэгдсэн `.prom` файл.

## Бүрэн товч мэдээлэл хэрэгтэй байна уу?

ADDR-2-ын бүрэн нийцлийн төлөв (эзэмшигч, хяналтын төлөвлөгөө, нээлттэй үйлдлийн зүйлүүд)
дагуу хадгалах сан дотор `docs/source/account_address_status.md`-д амьдардаг
RFC (`docs/account_structure.md`) хаягийн бүтэцтэй. Энэ хуудсыг ашиглана уу
шуурхай үйл ажиллагааны сануулга; Нарийвчилсан зааварчилгаа авахын тулд репо баримт бичгүүдийг хойшлуулна уу.