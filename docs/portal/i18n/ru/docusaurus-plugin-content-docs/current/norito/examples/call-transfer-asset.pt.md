---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пул: /norito/examples/call-transfer-asset
title: Invocar Transferencia do Host a Partir de Kotodama
описание: Используйте точку входа Kotodama, чтобы получить инструкции для хоста `transfer_asset` с валидацией встроенных метаданных.
источник: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Используя точку входа Kotodama, можно получить инструкции для хоста `transfer_asset` с подтверждением встроенных метаданных.

## Ротейру до Ливро Разау

- Финансирование авторитарного договора (например, `i105...`) как активная операция, требующая перевода и предоставление авторизации на бумаге `CanTransfer` или эквивалентного разрешения.
- Вызовите точку входа `call_transfer_asset` для передачи 5 сообщений из договора по контрато для `i105...`, отобразите как автоматический сетевой способ охвата хоста.
- Подтвердите вводы через `FindAccountAssets` или `iroha_cli ledger assets list --account i105...` и проверьте события, чтобы подтвердить защиту регистрации метаданных или контекста передачи.

## Рекомендации по использованию SDK

- [Краткий старт работы с SDK Rust](/sdks/rust)
- [Краткий старт работы с SDK Python](/sdks/python)
- [Быстрое начало работы с SDK JavaScript](/sdks/javascript)

[Вставьте шрифт Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```