---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6404e09aa8f3520328249a1d5c41309b291087908a2a8f5abae3e2fe12de44fb
source_last_modified: "2026-01-05T09:28:11.861409+00:00"
translation_last_reviewed: 2026-02-07
id: deal-engine
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: Overview of the SF-8 deal engine, Torii integration, and telemetry surfaces.
translator: machine-google-reviewed
---

::: Каноник эх сурвалжийг анхаарна уу
:::

# SoraFS Deal Engine

SF-8 замын газрын зураг нь SoraFS хэлцлийн хөдөлгүүрийг танилцуулж, хангах боломжийг олгодог.
хоорондын хадгалалт, олж авах гэрээний тодорхойлогч нягтлан бодох бүртгэл
үйлчлүүлэгч, үйлчилгээ үзүүлэгч. Гэрээг Norito ачааллын дагуу тайлбарласан болно
`crates/sorafs_manifest/src/deal.rs`-д тодорхойлсон бөгөөд хэлцлийн нөхцөл, бонд
түгжих, магадлалын бичил төлбөр, тооцооны бүртгэл.

Суулгасан SoraFS ажилтан (`sorafs_node::NodeHandle`) одоо
Зангилааны процесс бүрийн хувьд `DealEngine` жишээ. Хөдөлгүүр:

- `DealTermsV1` ашиглан хэлцлийг баталгаажуулах, бүртгэх;
- хуулбарлалтын ашиглалтыг мэдээлэх үед XOR-ээр тооцсон төлбөр хуримтлагддаг;
- детерминистик ашиглан магадлалын бичил төлбөрийн цонхыг үнэлдэг
  Блэйк3-д суурилсан дээж авах; болон
- засаглалд тохирсон бүртгэлийн агшин зуурын зураг болон төлбөр тооцооны ачааллыг гаргадаг
  хэвлэн нийтлэх.

Нэгжийн тест нь баталгаажуулалт, бичил төлбөрийн сонголт, төлбөр тооцооны урсгалыг хамардаг
операторууд API-г итгэлтэйгээр ашиглах боломжтой. Суурин газрууд одоо ялгардаг
`DealSettlementV1` удирдлагын ачааллыг SF-12 руу шууд холбох
хэвлэх шугам, `sorafs.node.deal_*` OpenTelemetry цувралыг шинэчлэх
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) Torii хяналтын самбар болон SLO-д зориулсан
хэрэгжилт. Дараах зүйлүүд нь аудиторын санаачилсан тайрах автоматжуулалт болон
цуцлах семантикийг засаглалын бодлоготой уялдуулах.

Ашиглалтын телеметр нь одоо `sorafs.node.micropayment_*` хэмжүүрийн багцыг тэжээдэг:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, тасалбарын тоолуур
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Эдгээр нийлбэрүүд нь магадлалыг харуулж байна
Сугалааны урсгал нь операторууд бичил төлбөрийн ялалт болон зээлийн шилжүүлгийг хооронд нь уялдуулах боломжтой
төлбөр тооцооны үр дүнтэй.

## Torii Интеграци

Torii нь тусгай зориулалтын төгсгөлийн цэгүүдийг ил гаргадаг тул үйлчилгээ үзүүлэгчид ашиглалтын талаар мэдээлж,
Захиалгат утасгүйгээр амьдралын мөчлөгийг зохицуулах:

- `POST /v2/sorafs/deal/usage` `DealUsageReport` телеметрийг хүлээн авч буцаана
  нягтлан бодох бүртгэлийн тодорхой үр дүн (`UsageOutcome`).
- `POST /v2/sorafs/deal/settle` одоогийн цонхыг дамжуулж дуусгана
  Үр дүнд нь `DealSettlementRecord`, base64 кодлогдсон `DealSettlementV1`-ийн хажууд
  засаглалын DAG хэвлэлд бэлэн.
- Torii-ийн `/v2/events/sse` хангамж одоо `SorafsGatewayEvent::DealUsage`-г цацаж байна
  хэрэглээ бүрийг хураангуйлсан бүртгэл (эрин үе, хэмжигдсэн ГиБ-цаг, тасалбар
  тоолуур, тодорхойлогч цэнэгүүд), `SorafsGatewayEvent::DealSettlement`
  каноник тооцооны дэвтэр агшин зуурын зураг дээр нэмсэн бүртгэлүүд
  Диск дээрх засаглалын олдворын BLAKE3 дижест/хэмжээ/суурь64, мөн
  `SorafsGatewayEvent::ProofHealth` нь PDP/PoTR босго өндөр байх бүрт дохио өгдөг.
  хэтэрсэн (үйлүүлэгч, цонх, ажил хаялт/хөргөх төлөв, торгуулийн хэмжээ). Хэрэглэгчид чадна
  Санал асуулгагүйгээр шинэ телеметр, тооцоолол эсвэл эрүүл мэндийн баталгааны сэрэмжлүүлэгт хариу өгөхийн тулд үйлчилгээ үзүүлэгчээр шүүнэ үү.

Хоёр төгсгөлийн цэг нь шинэ хувилбараар дамжуулан SoraFS квотын хүрээнд оролцдог.
`torii.sorafs.quota.deal_telemetry` цонх нь операторуудад тохируулах боломжийг олгодог
байршуулалт бүрт зөвшөөрөгдсөн илгээлтийн хувь хэмжээ.