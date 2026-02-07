---
lang: mn
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fcd909a7013c5147e4f0c89c67de856ff56797b99281b954c7708ad83ab5cdc8
source_last_modified: "2026-01-28T17:11:30.699790+00:00"
translation_last_reviewed: 2026-02-07
id: address-checksum-runbook
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
---

::: Каноник эх сурвалжийг анхаарна уу
Энэ хуудас нь `docs/source/sns/address_checksum_failure_runbook.md`-ийг толилуулж байна. Шинэчлэх
эхлээд эх файлыг, дараа нь энэ хуулбарыг синк хийнэ үү.
:::

Шалгалтын нийлбэр алдаа `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) хэлбэрээр гарч ирдэг.
Torii, SDK, хэтэвч/хайгуулын үйлчлүүлэгчид. Одоо ADDR-6/ADDR-7 замын газрын зураг
шалгах нийлбэрийн дохиолол эсвэл дэмжлэг болгонд энэ runbook-ийг дагаж мөрдөхийг операторуудаас шаардана
тасалбар шатаж байна.

## Жүжгийг хэзээ тоглох вэ

- **Сэрэмжлүүлэг:** `AddressInvalidRatioSlo` (д заасан
  `dashboards/alerts/address_ingest_rules.yml`) аялал болон тэмдэглэгээний жагсаалт
  `reason="ERR_CHECKSUM_MISMATCH"`.
- **Бэхэлгээний шилжилт:** `account_address_fixture_status` Prometheus текст файл эсвэл
  Grafana хяналтын самбар нь аливаа SDK хуулбарыг шалгах нийлбэр таарахгүй байгааг мэдээлдэг.
- **Дэмжлэгийн өсөлт:** Wallet/explorer/SDK багууд шалгах нийлбэрийн алдаа, IME-г иш татдаг
  авлига, эсвэл тайлахаа больсон санах ойн сканнерууд.
- **Гараар хийсэн ажиглалт:** Torii логууд `address_parse_error=checksum_mismatch` давтагдсаныг харуулж байна
  үйлдвэрлэлийн төгсгөлийн цэгүүдэд зориулагдсан.

Хэрэв осол нь ялангуяа Орон нутгийн-8/Орон нутгийн-12 мөргөлдөөний тухай бол дараахыг дагана уу
Оронд нь `AddressLocal8Resurgence` эсвэл `AddressLocal12Collision` тоглоомын дэвтэр.

## Нотлох баримт шалгах хуудас

| Нотлох баримт | Тушаал / Байршил | Тэмдэглэл |
|----------|-------------------|-------|
| Grafana хормын хувилбар | `dashboards/grafana/address_ingest.json` | Хүчингүй шалтгааны задаргаа болон нөлөөлөлд өртсөн төгсгөлийн цэгүүдийг авах. |
| Сэрэмжлүүлэг ачаа | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | Контекст шошго болон цагийн тэмдэглэгээг оруулна уу. |
| Бэхэлгээний эрүүл мэнд | `artifacts/account_fixture/address_fixture.prom` + Grafana | SDK хуулбарууд `fixtures/account/address_vectors.json`-ээс холдсон эсэхийг нотолно. |
| PromQL асуулга | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | Ослын баримт бичигт CSV экспортлох. |
| Бүртгэл | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (эсвэл лог нэгтгэх) | Хуваалцахаас өмнө PII-г цэвэрлэ. |
| Бэхэлгээний баталгаажуулалт | `cargo xtask address-vectors --verify` | Канон үүсгэгчийг баталгаажуулж, JSON зөвшөөрч байна. |
| SDK паритет шалгах | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | Сэрэмжлүүлэг/тасалбарт мэдээлэгдсэн SDK бүрийг ажиллуул. |
| Түр санах ой/IME эрүүл ухаан | `iroha tools address inspect <literal>` | Нуугдсан тэмдэгтүүд эсвэл IME дахин бичихийг илрүүлэх; `address_display_guidelines.md` иш тат. |

## Шууд хариу өгөх

1. Сэрэмжлүүлэгийг хүлээн зөвшөөрч, Grafana агшин зуурын + PromQL гаралтыг тохиолдлоор холбоно уу
   урсгал, тэмдэглэл нөлөөлсөн Torii контекст.
2. Хаягийг задлан шинжилж буй манифест сурталчилгаа / SDK хувилбаруудыг царцаах.
3. Хяналтын самбарын хормын хувилбарууд болон үүсгэсэн Prometheus текст файлын олдворуудыг дараах дотор хадгална уу.
   ослын хавтас (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. `checksum_mismatch` ачааллыг харуулсан бүртгэлийн дээжийг татах.
5. SDK эзэмшигчдэд (`#sdk-parity`) түүвэр ачааллын талаар мэдэгдээрэй.

## Үндсэн шалтгааныг тусгаарлах

### Бэхэлгээ эсвэл генераторын дрейф

- `cargo xtask address-vectors --verify`-г дахин ажиллуулах; амжилтгүй болбол дахин сэргээнэ.
- `ci/account_fixture_metrics.sh` (эсвэл хувь хүн
  `scripts/account_fixture_helper.py check`) багцыг баталгаажуулахын тулд SDK бүрийн хувьд
  бэхэлгээ нь каноник JSON-той таарч байна.

### Үйлчлүүлэгчийн кодлогч / IME регресс

- Хэрэглэгчийн өгсөн литералуудыг `iroha tools address inspect`-ээр шалгана уу.
  нэгдэх, кана хөрвүүлэлт, эсвэл таслагдсан ачаалал.
- Cross-check хэтэвч/explorer урсгал
  `docs/source/sns/address_display_guidelines.md` (хос хуулбарлах зорилтууд, анхааруулга,
  QR туслахууд) нь батлагдсан UX-ийг дагаж мөрддөг.

### Манифест эсвэл бүртгэлийн асуудал

- Хамгийн сүүлийн үеийн манифест багцыг дахин баталгаажуулахын тулд `address_manifest_ops.md`-г дагаж,
  Local-8 сонгогчийг дахин засаагүй эсэхийг шалгаарай.
  ачаалалд илэрдэг.

### Хортой эсвэл буруу хэлбэрийн траффик

- Torii бүртгэл болон `torii_http_requests_total`-ээр дамжуулан зөрчилтэй IP/app ID-г задлах.
- Аюулгүй байдал/Засаглалын хяналтад зориулж дор хаяж 24 цагийн бүртгэл хадгална.

## Нөлөөлөх ба сэргээх

| Хувилбар | Үйлдлүүд |
|----------|---------|
| Бэхэлгээний дрейф | `fixtures/account/address_vectors.json`-г сэргээж, `cargo xtask address-vectors --verify`-г дахин ажиллуулж, SDK багцуудыг шинэчилж, `address_fixture.prom` агшин зуурын зургийг тасалбарт хавсаргана уу. |
| SDK/клиент регресс | Каноник бэхэлгээ + `iroha tools address inspect` гаралт болон SDK паритын CI (жишээ нь, `ci/check_address_normalize.sh`)-ийн ард гарсан хаалганы хувилбаруудтай холбоотой файлын асуудлууд. |
| Хортой мэдүүлэг | Зөрчил гаргасан захирлуудын хувь хэмжээг хязгаарлах эсвэл блоклох, хэрэв булшны чулуу сонгогч шаардлагатай бол Засаглал руу шилжүүлнэ үү. |

Хөнгөвчлөх арга хэмжээ авсны дараа баталгаажуулахын тулд дээрх PromQL асуулгыг дахин ажиллуулна уу
`ERR_CHECKSUM_MISMATCH` хамгийн багадаа тэг (`/tests/*`-ээс бусад) хэвээр байна
Үйл явдлыг бууруулахаас 30 минутын өмнө.

## Хаалт

1. Grafana агшин зуурын зураг, PromQL CSV, бүртгэлийн ишлэл, `address_fixture.prom` архивыг хадгал.
2. Хэрэгсэл/баримт бичиг байгаа бол `status.md` (ADDR хэсэг) болон замын зургийн мөрийг шинэчилнэ үү.
   өөрчлөгдсөн.
3. Шинэ хичээл орох үед тохиолдлын дараах тэмдэглэлийг `docs/source/sns/incidents/` доор файллаарай
   гарч ирэх.
4. SDK хувилбарын тэмдэглэлд шалгах нийлбэрийн засваруудыг шаардлагатай үед дурдсан эсэхийг шалгаарай.
5. Сэрэмжлүүлэг 24 цагийн турш ногоон хэвээр байх ба бэхэлгээний шалгалт өмнө нь ногоон хэвээр байхыг баталгаажуулна уу
   шийдвэрлэх.