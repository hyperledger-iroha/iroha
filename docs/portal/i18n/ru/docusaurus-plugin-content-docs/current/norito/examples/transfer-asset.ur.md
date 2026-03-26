---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слаг: /norito/examples/transfer-asset
Название: اکاؤنٹس کے درمیان اثاثہ منتقل کریں
описание: Краткое руководство по использованию SDK и пошаговые руководства по использованию SDK.
источник: example/transfer/transfer.ko
---

Ознакомьтесь с краткими руководствами по SDK и пошаговыми руководствами, которые помогут вам в этом.

## لیجر واک تھرو

- Алиса сказала, что может использовать SDK для быстрого запуска (например, `register and mint`). ذریعے)۔
- `do_transfer` Если вам нужна помощь Алисы и Боба, вам нужно 10 дней в году. `AssetTransferRole` اجازت پوری ہو۔
- Проверьте (`FindAccountAssets`, `iroha_cli ledger assets list`) تاکہ ٹرانسفر کے نتیجے کا مشاہدہ ہو۔

## Использование SDK

- [Краткий старт Rust SDK](/sdks/rust)
- [Краткий старт Python SDK](/sdks/python)
- [Краткое руководство по JavaScript SDK] (/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```