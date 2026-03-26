---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/ejemplos/transfer-asset
título: اکاؤنٹس کے درمیان اثاثہ منتقل کریں
descripción: Inicio rápido del SDK y tutoriales del SDK کی عکاسی کرتا ہے۔
fuente: ejemplos/transfer/transfer.ko
---

سادہ اثاثہ ٹرانسفر ورک فلو جو SDK Quickstarts اور لیجر tutoriales کی عکاسی کرتا ہے۔

## لیجر واک تھرو

- Alice کو ہدف اثاثہ پہلے سے فنڈ کریں (مثلا `register and mint` اسنیپٹ یا SDK inicio rápido فلو کے ذریعے)۔
- `do_transfer` انٹری پوائنٹ چلائیں تاکہ Alice سے Bob کو 10 یونٹس منتقل ہوں، اور `AssetTransferRole` اجازت پوری ہو۔
- بیلنس (`FindAccountAssets`, `iroha_cli ledger assets list`) چیک کریں یا پائپ لائن ایونٹس سبسکرائب کریں تاکہ ٹرانسفر کے نتیجے کا مشاہدہ ہو۔

## متعلقہ SDK گائیڈز

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

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