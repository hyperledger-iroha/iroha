---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Зах зээлийн чадавхийг баталгаажуулах хяналтын хуудас

**Хянах цонх:** 2026-03-18 → 2026-03-24  
**Хөтөлбөрийн эзэд:** Хадгалах баг (`@storage-wg`), Засаглалын зөвлөл (`@council`), Төрийн сангийн холбоо (`@treasury`)  
**Хамрах хүрээ:** SF-2c GA-д шаардлагатай нийлүүлэгчийн шугам хоолой, маргааныг хянан шийдвэрлэх урсгал болон төрийн сангийн нэгтгэх үйл явц.

Гадны операторуудын зах зээлийг идэвхжүүлэхийн өмнө доорх хяналтын хуудсыг хянаж үзэх шаардлагатай. Мөр бүр нь аудиторуудын дахин тоглуулж болох тодорхой нотолгоог (туршилт, бэхэлгээ эсвэл баримт бичиг) холбодог.

## Хүлээн авах хяналтын хуудас

### Үйлчилгээ үзүүлэгчийн элсэлт

| Шалгах | Баталгаажуулалт | Нотлох баримт |
|-------|------------|----------|
| Бүртгэл нь каноник хүчин чадлын мэдэгдлийг хүлээн авдаг | Интеграцийн тест нь `/v2/sorafs/capacity/declare` программын API-ээр дамжуулан гарын үсэг боловсруулах, мета өгөгдөл авах, зангилааны бүртгэлд шилжүүлэх зэргийг баталгаажуулдаг. | `crates/iroha_torii/src/routing.rs:7654` |
| Ухаалаг гэрээ нь таарахгүй ачааллаас татгалздаг | Нэгжийн тест нь үйлчилгээ үзүүлэгчийн ID болон хүлээгдэж буй GiB талбарууд нь гарын үсэг зурсан мэдэгдэлтэй тохирч байгаа эсэхийг баталгаажуулдаг. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI нь каноник холболтын олдворуудыг ялгаруулдаг | CLI бэхэлгээ нь тодорхойлогч Norito/JSON/Base64 гаралтыг бичиж, хоёр талын аяллыг баталгаажуулдаг тул операторууд мэдэгдлүүдийг офлайнаар гаргах боломжтой. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Операторын гарын авлага нь элсэлтийн ажлын явц болон засаглалын хамгаалалтын хашлагуудыг агуулна | Баримт бичигт тунхаглалын схем, бодлогын өгөгдмөл байдал, зөвлөлийн хянан шалгах алхмуудыг жагсаав. | `../storage-capacity-marketplace.md` |

### Маргаан шийдвэрлэх

| Шалгах | Баталгаажуулалт | Нотлох баримт |
|-------|------------|----------|
| Маргааны бүртгэлүүд нь каноник ачааллын хураамж | Нэгжийн тест нь маргааныг бүртгэж, хадгалагдсан ачааллын кодыг тайлж, дэвтэрийн детерминизмыг баталгаажуулахын тулд хүлээгдэж буй статусыг баталгаажуулдаг. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI маргаан үүсгэгч нь каноник схемтэй таарч байна | CLI тест нь `CapacityDisputeV1`-д зориулсан Base64/Norito гаралт болон JSON хураангуйг хамарч, нотлох баримтын багц хэшийг тодорхой болгодог. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Дахин тоглуулах тест нь маргаан/торгуулийн детерминизмыг нотолж байна | Бүтэлгүйтлийн нотлох телеметрийг хоёр удаа дахин тоглуулснаар ижил дэвтэр, кредит болон маргааны агшин зуурын зургуудыг гаргадаг тул налуу зураас нь үе тэнгийнхний дунд тодорхойлогддог. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook баримт бичгийн өсөлт болон хүчингүй болгох урсгал | Үйл ажиллагааны гарын авлага нь зөвлөлийн ажлын урсгал, нотлох баримтын шаардлага, буцаах журам зэргийг багтаасан болно. | `../dispute-revocation-runbook.md` |

### Төрийн сангийн зохицуулалт

| Шалгах | Баталгаажуулалт | Нотлох баримт |
|-------|------------|----------|
| Бүртгэлийн аккруэл 30 хоногийн нэвт норгох төсөөлөлтэй таарч байна | Soak test нь тооцооны 30 цонхны дагуу таван үйлчилгээ үзүүлэгчийг хамардаг бөгөөд тооцооны дэвтэрийн бичилтүүд хүлээгдэж буй төлбөрийн лавлагаатай ялгаатай. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Бүртгэлийн дансны экспортын тохирол шөнө бүр бүртгэгдсэн | `capacity_reconcile.py` нь хураамжийн дэвтрийн хүлээлтийг гүйцэтгэсэн XOR шилжүүлгийн экспорттой харьцуулж, Prometheus хэмжигдэхүүнийг ялгаруулж, Alertmanager-ээр дамжуулан төрийн сангийн зөвшөөрлийг өгдөг. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Тооцооны самбарууд торгууль болон хуримтлалын телеметр | Grafana импортын график GiB·цагын хуримтлал, strike counters болон дуудлага дээр харагдахуйц баталгаатай барьцаа. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Нийтлэгдсэн тайлангийн архивууд аргачлал болон дахин тоглуулах командуудыг нэвт норгох | Тайлангийн дэлгэрэнгүй мэдээлэл нь аудиторуудад зориулсан хамрах хүрээ, гүйцэтгэх командууд, ажиглалтын дэгээ зэргийг багтаасан болно. | `./sf2c-capacity-soak.md` |

## Гүйцэтгэлийн тэмдэглэл

Гарахаасаа өмнө баталгаажуулалтын багцыг дахин ажиллуулна уу:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Операторууд залгах/маргааны хүсэлтийн ачааллыг `sorafs_manifest_stub capacity {declaration,dispute}` ашиглан дахин үүсгэж, JSON/Norito байтыг удирдлагын тасалбарын хажуугаар архивлах ёстой.

## Бүртгүүлэх олдворууд

| Олдвор | Зам | blake2b-256 |
|----------|------|-------------|
| Үйлчилгээ үзүүлэгчийн элсэлтийн зөвшөөрлийн багц | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Маргаан шийдвэрлэх зөвшөөрлийн багц | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Төрийн сангийн тохирлын зөвшөөрлийн багц | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Эдгээр олдворуудын гарын үсэгтэй хуулбарыг багцын хамт хадгалж, засаглалын өөрчлөлтийн бүртгэлд холбоно уу.

## Зөвшөөрөл

- Хадгалах багийн ахлагч — @storage-tl (2026-03-24)  
- Засаглалын зөвлөлийн нарийн бичгийн дарга — @council-sec (2026-03-24)  
- Төрийн сангийн үйл ажиллагааны тэргүүлэгч — @treasury-ops (2026-03-24)