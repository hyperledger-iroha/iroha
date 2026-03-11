---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слаг: /norito/examples/transfer-asset
Название: Transferir ativo entre contas
описание: Fluxo direto de Transferencia de Ativos que espelha os faststarts do SDK и os roteiros do livro razao.
источник: example/transfer/transfer.ko
---

Fluxo Directo de Transferencia de Ativos que espelha os faststarts for SDK и os roteiros do livro razao.

## Ротейру до Ливро Разау

- Предварительное финансирование Алисы в активном режиме (например, через файл `register and mint` или потоки быстрого запуска SDK).
- Выполните точку входа `do_transfer` для перемещения 10 раз Алисы для Боба, затем получите разрешение `AssetTransferRole`.
- Проконсультируйтесь по ссылкам (`FindAccountAssets`, `iroha_cli ledger assets list`) или другим событиям в конвейере, чтобы наблюдать за результатами передачи.

## Рекомендации по использованию SDK

- [Краткий старт работы с SDK Rust](/sdks/rust)
- [Краткий старт работы с SDK Python](/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Вставьте шрифт Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```