<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 479d552d0f641875518c62059be1084af6ddf99213662a753c73ea57512b8e5f
source_last_modified: "2026-04-02T18:24:28.189405+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/transfer-asset
title: Данс хооронд хөрөнгө шилжүүлэх
description: SDK-н шуурхай эхлэл болон дэвтэрийн алхмуудыг тусгадаг шууд хөрөнгө шилжүүлэх ажлын урсгал.
source: examples/transfer/transfer.ko
---

SDK-н шуурхай эхлэл болон дэвтэрийн алхмуудыг тусгадаг шууд хөрөнгө шилжүүлэх ажлын урсгал.

## Бүртгэлийн дэвтэр

- Алисыг зорилтот хөрөнгөөр нь урьдчилан санхүүжүүлэх (жишээ нь "бүртгүүл, гаа" гэсэн хэсэг эсвэл SDK-ийн хурдан эхлүүлэх урсгалаар).
- `AssetTransferRole` зөвшөөрлийг хангаж, Алисаас Боб руу 10 нэгж шилжүүлэхийн тулд `do_transfer` нэвтрэх цэгийг ажиллуулна уу.
- Үлдэгдэл (`FindAccountAssets`, `iroha_cli ledger asset list`) асуух эсвэл дамжуулах үр дүнг ажиглахын тулд дамжуулах хоолойн үйл явдалд бүртгүүлнэ үү.

## Холбогдох SDK гарын авлага

- [Зэв SDK хурдан эхлүүлэх](/sdks/rust)
- [Python SDK хурдан эхлүүлэх](/sdks/python)
- [JavaScript SDK хурдан эхлүүлэх](/sdks/javascript)

[Kotodama эх сурвалжийг татаж авах](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7"),
      account!("sorauロ1NksツJZミLツスjヨrUphCSホ8Wノスマチモr3ムLセヌヒYqwフノFTMDQE"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```