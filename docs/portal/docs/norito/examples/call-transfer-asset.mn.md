<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dcd8de175a7c5172158a03e1a25b254c90a11e62c173f95b8d9e4a387df6ba09
source_last_modified: "2026-03-26T13:01:47.372931+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/call-transfer-asset
title: Kotodama-аас хост шилжүүлгийг дуудах
description: Kotodama нэвтрэх цэг нь `transfer_asset` зааварчилгааг доторлогооны мета өгөгдлийн баталгаажуулалтаар хэрхэн дуудаж болохыг харуулж байна.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Kotodama нэвтрэх цэг нь `transfer_asset` зааварчилгааг доторлогооны мета өгөгдлийн баталгаажуулалтаар хэрхэн дуудаж болохыг харуулж байна.

## Бүртгэлийн дэвтэр

- Гэрээний байгууллагыг (жишээ нь гэрээний дансны хувьд `<i105-account-id>`) шилжүүлж, `CanTransfer` үүрэг эсвэл түүнтэй адилтгах зөвшөөрлийг олгох хөрөнгөөр санхүүжүүлнэ.
- Гэрээний данснаас 5 нэгжийг Боб (`<i105-account-id>`) руу шилжүүлэхийн тулд `call_transfer_asset` нэвтрэх цэг рүү залгаж, сүлжээн дэх автоматжуулалт нь хост дуудлагыг ороож чадах арга замыг тусгана.
- `FindAccountAssets` эсвэл `iroha_cli ledger asset list --account <i105-account-id>`-ээр дамжуулан үлдэгдлийг шалгаж, дамжуулалтын контекстийг бүртгэсэн мета өгөгдлийн хамгаалалтыг баталгаажуулахын тулд үйл явдлуудыг шалгана уу.

## Холбогдох SDK гарын авлага

- [Зэв SDK хурдан эхлүүлэх](/sdks/rust)
- [Python SDK хурдан эхлүүлэх](/sdks/python)
- [JavaScript SDK хурдан эхлүүлэх](/sdks/javascript)

[Kotodama эх сурвалжийг татаж авах](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```