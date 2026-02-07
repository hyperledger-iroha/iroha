---
lang: mn
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1be9268784bf75c4c5d1bf854e72c817475a079c0d2bf06ce120ccd325ad6083
source_last_modified: "2026-01-22T14:45:01.248924+00:00"
translation_last_reviewed: 2026-02-07
id: payment-settlement-plan
title: SNS Payment & Settlement Plan
sidebar_label: Payment & settlement plan
description: Playbook for routing SNS registrar revenue, reconciling steward/treasury splits, and producing evidence bundles.
translator: machine-google-reviewed
---

> Каноник эх сурвалж: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

Замын зургийн даалгавар **SN-5 — Төлбөр тооцооны үйлчилгээ** нь детерминистикийг танилцуулж байна
Sora Name Service-ийн төлбөрийн давхарга. Бүртгэл, сунгалт, буцаан олголт бүр
Norito бүтэцтэй ачааллыг ялгаруулах ёстой бөгөөд ингэснээр төрийн сан, нярав, засаглал
хүснэгтгүйгээр санхүүгийн урсгалыг дахин тоглуулах. Энэ хуудас нь техникийн үзүүлэлтүүдийг нэрлэж байна
портал үзэгчдэд зориулсан.

## Орлогын загвар

- Үндсэн хураамж (`gross_fee`) нь бүртгэгчийн үнийн матрицаас гардаг.  
- Төрийн сан `gross_fee × 0.70`, няравууд үлдсэнийг нь хасна.
  лавлагааны урамшуулал (10% -иар хязгаарлагдсан).  
- Нэмэлт сааталууд нь маргааны үед менежерийн төлбөрийг түр зогсоох боломжийг засаглалд олгодог.  
- Суурилуулалтын багцууд нь `ledger_projection` блокыг бетонтой хамт ил гаргадаг
  `Transfer` ISI нь автоматжуулалт нь XOR хөдөлгөөнийг шууд Torii руу оруулах боломжтой.

## Үйлчилгээ ба автоматжуулалт

| Бүрэлдэхүүн хэсэг | Зорилго | Нотлох баримт |
|----------|---------|----------|
| `sns_settlementd` | Бодлого, тэмдэгт багц, гадаргуу `/v1/sns/settlements`. | JSON багц + хэш. |
| Төлбөрийн дараалал & бичигч | Idempotent дараалал + `iroha_cli app sns settlement ledger`-ээр удирдуулсан дэвтэр илгээгч. | Багцын хэш ↔ tx хэш манифест. |
| Эвлэрүүлэн зуучлах ажил | `docs/source/sns/reports/` дор өдөр тутмын зөрүү + сарын тайлан. | Markdown + JSON тойм. |
| Буцаан олголтын ширээ | `/settlements/{id}/refund`-ээр дамжуулан засаглалаар батлагдсан буцаан олголт. | `RefundRecordV1` + тасалбар. |

CI туслахууд эдгээр урсгалыг тусгадаг:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Ажиглалт ба тайлагнах

- Хяналтын самбар: Төрийн сангийн хувьд `dashboards/grafana/sns_payment_settlement.json`
  няравын нийт дүн, лавлагааны төлбөр, дарааллын гүн, буцаан олголтын саатал.
- Анхааруулга: `dashboards/alerts/sns_payment_settlement_rules.yml` мониторууд хүлээгдэж байна
  нас, эвлэрлийн алдаа, дэвтэр зөрөх.
- Мэдэгдэл: өдөр тутмын мэдээ (`settlement_YYYYMMDD.{json,md}`) сар бүр эргэлддэг
  тайлангууд (`settlement_YYYYMM.md`) Git болон
  засаглалын объектын дэлгүүр (`s3://sora-governance/sns/settlements/<period>/`).
- Удирдлагын багцууд нь хяналтын самбар, CLI бүртгэл, зөвлөлөөс зөвшөөрөл авдаг.
  гарын үсэг зурах.

## Өргөдлийг шалгах хуудас

1. Үнийн санал + дэвтэрийн туслахуудыг загварчилж, үе шатны багцыг аваарай.
2. Дараалал + бичигч, утсан хяналтын самбар, дасгалын хамт `sns_settlementd`-г ажиллуулна уу
   дохиоллын туршилтууд (`promtool test rules ...`).
3. Буцаан олголтын туслах ажилтан, сар бүрийн тайлангийн загварыг хүргэх; олдворуудыг толин тусгах
   `docs/portal/docs/sns/reports/`.
4. Түншийн сургуулилт (төлбөр тооцооны бүтэн сар) гүйлгэнэ
   SN-5-ыг бүрэн гэж тэмдэглэсэн засаглалын санал хураалт.

Тодорхой схемийн тодорхойлолтыг эх баримтаас буцаан харна уу
асуултууд, цаашдын нэмэлт өөрчлөлтүүд.