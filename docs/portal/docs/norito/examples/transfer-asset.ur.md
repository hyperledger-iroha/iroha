---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0541f1f5775744c518f4f102326e725d73043b1756bb62a979f8eed4cc9472e6
source_last_modified: "2026-04-08T09:19:38.795296+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/transfer-asset
title: اکاؤنٹس کے درمیان اثاثہ منتقل کریں
description: سادہ اثاثہ ٹرانسفر ورک فلو جو SDK quickstarts اور لیجر walkthroughs کی عکاسی کرتا ہے۔
source: examples/transfer/transfer.ko
---

سادہ اثاثہ ٹرانسفر ورک فلو جو SDK quickstarts اور لیجر walkthroughs کی عکاسی کرتا ہے۔

## لیجر واک تھرو

- Alice کو ہدف اثاثہ پہلے سے فنڈ کریں (مثلا `register and mint` اسنیپٹ یا SDK quickstart فلو کے ذریعے)۔
- `do_transfer` انٹری پوائنٹ چلائیں تاکہ Alice سے Bob کو 10 یونٹس منتقل ہوں، اور `AssetTransferRole` اجازت پوری ہو۔
- بیلنس (`FindAccountAssets`, `iroha ledger asset list all --verbose`) چیک کریں یا پائپ لائن ایونٹس سبسکرائب کریں تاکہ ٹرانسفر کے نتیجے کا مشاہدہ ہو۔

## متعلقہ SDK گائیڈز

- [Rust SDK quickstart](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
