---
lang: mn
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Дансны хаягийн нийцлийн төлөв (ADDR-2)

Статус: 2026-03-30-нд хүлээн зөвшөөрөгдсөн  
Эзэмшигчид: Дата загварын баг / QA Guild  
Замын зургийн лавлагаа: ADDR-2 — Хос форматтай нийцлийн багц

### 1. Тойм

- Бэхэлгээ: `fixtures/account/address_vectors.json` (IH58 (давуу) + шахсан (`sora`, хоёрдугаарт) + multisig эерэг/сөрөг тохиолдол).
- Хамрах хүрээ: далд-өгөгдмөл, Локал-12, Глобал бүртгэл, алдааны бүрэн ангиллыг агуулсан multisig хянагчдыг хамарсан тодорхойлогч V1 ачаалал.
- Түгээлт: Rust дата-загвар, Torii, JS/TS, Swift, Android SDK дээр хуваалцсан; Аливаа хэрэглэгч хазайсан тохиолдолд CI амжилтгүй болно.
- Үнэний эх сурвалж: генератор нь `crates/iroha_data_model/src/account/address/compliance_vectors.rs`-д амьдардаг бөгөөд `cargo xtask address-vectors`-ээр дамжин илэрдэг.
### 2. Нөхөн сэргээх & Баталгаажуулах

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Тугнууд:

- `--out <path>` — түр зуурын багц үүсгэх үед нэмэлтээр дарах (өгөгдмөл нь `fixtures/account/address_vectors.json`).
- `--stdout` — диск рүү бичихийн оронд stdout руу JSON ялгаруулна.
- `--verify` — одоогийн файлыг шинээр үүсгэсэн контенттой харьцуулах (дрифт дээр хурдан бүтэлгүйтдэг; `--stdout`-д ашиглах боломжгүй).

### 3. Олдворын матриц

| Гадаргуу | Хэрэгжүүлэх | Тэмдэглэл |
|---------|-------------|-------|
| Rust өгөгдлийн загвар | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON-г задлан шинжилж, каноник ачааллыг сэргээж, IH58 (давуу)/шахсан (`sora`, хоёрдугаарт)/каноник хөрвүүлэлт + бүтцийн алдааг шалгана. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Сервер талын кодлогчийг баталгаажуулдаг тул Torii нь алдаатай IH58 (давуу)/шахсан (`sora`, хоёр дахь шилдэг) ачааллаас тодорхой татгалздаг. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Mirrors V1 бэхэлгээ (IH58 илүүд үздэг/шахсан (`sora`) хоёр дахь шилдэг/бүтэн өргөн) ба сөрөг тохиолдол бүрт Norito загварын алдааны кодыг баталгаажуулдаг. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Apple-ийн платформ дээр IH58 (давуу)/шахсан (`sora`, хоёр дахь шилдэг) код тайлах, олон тооны ачааллын ачаалал болон алдааг арилгах дасгалуудыг хийдэг. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Котлин/Жава холболтууд нь каноник бэхэлгээтэй нийцэж байгаа эсэхийг баталгаажуулдаг. |

### 4. Хяналт & Гайхалтай ажил- Статусын тайлагнах: энэ баримт бичиг нь `status.md` болон замын зурагтай холбоотой тул долоо хоног бүрийн тойм нь бэхэлгээний эрүүл мэндийг шалгах боломжтой.
- Хөгжүүлэгчийн порталын хураангуй: **Лавлагаа → Дансны хаягийн нийцэл**-ийг баримтын портал (`docs/portal/docs/reference/account-address-status.md`) дээрээс гадна талаас нь харж болно.
- Prometheus болон хяналтын самбарууд: SDK хуулбарыг баталгаажуулах бүрдээ `--metrics-out` (мөн сонголтоор `--metrics-label`) ашиглан туслагчийг ажиллуулснаар Prometheus текст файл цуглуулагч Prometheus-г залгих боломжтой. Grafana хяналтын самбар **Бүртгэлийн хаягийн бэхэлгээний төлөв** (`dashboards/grafana/account_address_fixture_status.json`) нь гадаргуу бүрт тэнцсэн/бүтэлгүйтлийн тоог гаргаж, аудитын нотлох баримтыг SHA-256 стандартын дагуу гаргадаг. Аливаа зорилтот `0`-г мэдээлэх үед сэрэмжлүүлнэ.
- Torii хэмжигдэхүүн: `torii_address_domain_total{endpoint,domain_kind}` одоо `torii_address_invalid_total`/`torii_address_local8_total` толин тусгалтай амжилттай задлан шинжлэгдсэн бүртгэл бүрт ялгардаг. Үйлдвэрлэлийн аливаа `domain_kind="local12"` замын хөдөлгөөний талаар сэрэмжлүүлж, тоолуурыг SRE `address_ingest` хяналтын самбарт тусгаснаар Local-12 тэтгэврийн хаалга нь шалгах боломжтой нотлох баримттай болно.
- Бэхэлгээний туслах: `scripts/account_fixture_helper.py` каноник JSON-г татаж эсвэл баталгаажуулдаг тул SDK хувилбарын автоматжуулалт нь Prometheus хэмжигдэхүүнийг бичих явцад гараар хуулах/буулгахгүйгээр багцыг авч/шалгах боломжтой. Жишээ:

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

  Туслах нь зорилтот таарч байгаа үед `account_address_fixture_check_status{target="android"} 1`, дээр нь SHA-256 задлах `account_address_fixture_remote_info` / `account_address_fixture_local_info` хэмжигчийг бичдэг. Алга болсон файлуудын тайлан `account_address_fixture_local_missing`.
  Автоматжуулалтын боодол: нэгдсэн текст файлыг гаргахын тулд cron/CI-ээс `ci/account_fixture_metrics.sh` руу залгаад (өгөгдмөл `artifacts/account_fixture/address_fixture.prom`). Давтагдсан `--target label=path` оруулгуудыг нэвтрүүлэх (сонголт болгонд `::https://mirror/...`-г нэмж эх сурвалжийг дарж бичих) Prometheus нь SDK/CLI хуулбар бүрийг хамарсан нэг файлыг хусдаг. GitHub ажлын урсгал `address-vectors-verify.yml` нь энэ туслахыг аль хэдийн каноник бэхэлгээний эсрэг ажиллуулж, SRE залгихад зориулж `account-address-fixture-metrics` олдворыг байршуулдаг.