---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слаг: /norito/examples/transfer-asset
title: Перевести активы между аккаунтами
описание: Простой сценарий перевода активов, отражающий Quickstart'ы SDK и пошаговое руководство'ы реестра.
источник: example/transfer/transfer.ko
---

Простой сценарий перевода активов, конвертирующий Quickstart'ы SDK и пошаговое руководство'ы реестра.

## Пошаговый обход реестра

- Предварительно заполните Алису целевым активом (например, через фрагмент `register and mint` или потоки быстрого запуска SDK).
- Выполните точку входа `do_transfer`, чтобы перевести 10 единиц от Алисы к Бобу, чтобы обеспечить разрешение `AssetTransferRole`.
- Проверьте балансы (`FindAccountAssets`, `iroha_cli ledger assets list`) или запишите события конвейера, наблюдайте за передачей результата.

## Связанные управления SDK

- [Быстрый запуск Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Быстрый запуск JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/transfer-asset.ko)

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