---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пул: /norito/examples/call-transfer-asset
title: Запрос на передачу хоста от Kotodama
описание: Если вы хотите использовать точку входа Kotodama, вы можете воспользоваться инструкциями хоста `transfer_asset` по проверке метаданных в режиме онлайн.
источник: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Если вы хотите использовать точку входа Kotodama, вы можете воспользоваться инструкциями хоста `transfer_asset` по проверке метаданных в режиме онлайн.

## Запись мэра библиотеки

- Назначьте авторизацию договора (например, `<i105-account-id>`) с активацией, которая передает и отменяет роль `CanTransfer` или эквивалентное разрешение.
- Назовите точку входа `call_transfer_asset` для передачи 5 единиц от места контракта на `<i105-account-id>`, отобразив форму, в которой автоматизация в цепочке может охватывать сигналы хоста.
- Проверка балансов медианте `FindAccountAssets` или `iroha_cli ledger assets list --account <i105-account-id>` и проверка событий для подтверждения того, что охраняется регистрация метаданных в контексте передачи.

## Руководство по настройке SDK

- [Краткий запуск SDK de Rust](/sdks/rust)
- [Краткий запуск SDK Python](/sdks/python)
- [Краткий запуск SDK JavaScript](/sdks/javascript)

[Удалить ссылку Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```