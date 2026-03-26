---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пул: /norito/examples/call-transfer-asset
Название: استدعاء نقل المضيف من Kotodama
описание: يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع Это произошло в результате событий в Сан-Франциско.
источник: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Он был отправлен в США Kotodama, чтобы установить `transfer_asset`. Это произошло в результате событий в Сан-Франциско.

## جولة دفتر الأستاذ

- Добавлено сообщение (`i105...`) Да, это правда.
- Установите флажок `call_transfer_asset` в течение 5 дней с момента запуска `i105...`, Он сказал, что хочет, чтобы он сделал это.
- تحقق من الأرصدة عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account i105...` وافحص الأحداث لتأكيد حارس Дэнни Сейл Сейлл.

## Использование SDK

- [Загрузка в Rust SDK](/sdks/rust)
- [Просмотр Python SDK](/sdks/python)
- [Загрузка JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/call-transfer-asset.ko)

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