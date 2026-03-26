---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пул: /norito/examples/call-transfer-asset
title: Kotodama سے ہوسٹ ٹرانسفر کال کریں
описание: دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن Если вы хотите использовать встроенные функции, вы можете использовать их в качестве встроенного варианта.
источник: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Если вам нужен Kotodama, вы можете использовать `transfer_asset`. встроенные функции, которые помогут вам выбрать нужный вариант

## لیجر واک تھرو

- Установите флажок (`i105...`) для получения дополнительной информации о Если у вас есть `CanTransfer`, вы можете использовать его в качестве источника питания.
- `call_transfer_asset` Для получения дополнительной информации о `i105...` для 5 یونٹس منتقل ہوں، یہ اس طریقے کی عکاسی کرتا ہے کہ کین آٹومیشن ہوسٹ کالز کو لپیٹ سکتی ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account i105...` может быть использовано в качестве дополнительного источника питания. Если вы хотите, чтобы вы выбрали лучший вариант

## Использование SDK

- [Краткий старт Rust SDK](/sdks/rust)
- [Краткое руководство по Python SDK] (/sdks/python)
- [Краткое руководство по JavaScript SDK] (/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```