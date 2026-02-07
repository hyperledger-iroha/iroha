---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слаг: /norito/examples/transfer-asset
Название: Transferer un actif entre comptes
описание: Простой поток переноса действий, который отражает SDK быстрого запуска и парки регистрации.
источник: example/transfer/transfer.ko
---

Простой процесс переноса действий, который отражает быстрые запуски SDK и парки регистрации.

## Парк регистрации

- Préfinancez Alice с активным доступом (например, через фрагмент `register and mint` или поток быстрого запуска SDK).
- Выполните точку входа `do_transfer` для перемещения 10 единиц Алисы и Боба, получив разрешение `AssetTransferRole`.
- Опросите солдаты (`FindAccountAssets`, `iroha_cli ledger assets list`) или подключитесь к другим объектам конвейера для наблюдения за результатами передачи.

## Руководства для партнеров SDK

- [Быстрый запуск SDK Rust](/sdks/rust)
- [Быстрый запуск SDK Python] (/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Зарядное устройство источника Kotodama](/norito-snippets/transfer-asset.ko)

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