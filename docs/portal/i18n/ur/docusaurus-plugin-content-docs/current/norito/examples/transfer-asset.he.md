---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ecd5cd63fd38efc80026bf9795c910fa9f45c329a71e9a5ef39671d12bb738b8
source_last_modified: "2025-11-14T04:43:20.812983+00:00"
translation_last_reviewed: 2026-01-30
---

سادہ اثاثہ ٹرانسفر ورک فلو جو SDK quickstarts اور لیجر walkthroughs کی عکاسی کرتا ہے۔

## لیجر واک تھرو

- Alice کو ہدف اثاثہ پہلے سے فنڈ کریں (مثلا `register and mint` اسنیپٹ یا SDK quickstart فلو کے ذریعے)۔
- `do_transfer` انٹری پوائنٹ چلائیں تاکہ Alice سے Bob کو 10 یونٹس منتقل ہوں، اور `AssetTransferRole` اجازت پوری ہو۔
- بیلنس (`FindAccountAssets`, `iroha_cli ledger assets list`) چیک کریں یا پائپ لائن ایونٹس سبسکرائب کریں تاکہ ٹرانسفر کے نتیجے کا مشاہدہ ہو۔

## متعلقہ SDK گائیڈز

- [Rust SDK quickstart](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
