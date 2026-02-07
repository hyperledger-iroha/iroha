---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слаг: /norito/examples/transfer-asset
Название: Transferir activo entre cuentas
описание: Flujo Directo de Transferencia de Activos que refleja los Quickstarts de los SDK и los recorridos del libro mayor.
источник: example/transfer/transfer.ko
---

Flujo Directo de Transferencia de Activos que отражает быстрые старты SDK и записи libro mayor.

## Запись мэра библиотеки

- Предварительно найдите Алису с активным объектом (например, с помощью фрагмента `register and mint` или потоков быстрого запуска SDK).
- Выведите точку входа `do_transfer` для перемещения 10 раз от Алисы и Боба, получив разрешение `AssetTransferRole`.
- Просмотрите балансы (`FindAccountAssets`, `iroha_cli ledger assets list`) или подпишитесь на события конвейера, чтобы наблюдать за результатами перевода.

## Руководство по настройке SDK

- [Краткий запуск SDK Rust](/sdks/rust)
- [Краткий запуск SDK Python] (/sdks/python)
- [Краткий запуск SDK JavaScript](/sdks/javascript)

[Удалить ссылку Kotodama](/norito-snippets/transfer-asset.ko)

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