---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd3de67c0ff1f0982a38cc5cb237fceba884e2788f0c4f48e1632e0cbfc340b2
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/transfer-asset
title: Перевести актив между аккаунтами
description: Простой сценарий перевода активов, отражающий quickstart'ы SDK и walkthrough'ы реестра.
source: examples/transfer/transfer.ko
---

Простой сценарий перевода активов, отражающий quickstart'ы SDK и walkthrough'ы реестра.

## Пошаговый обход реестра

- Предварительно пополните Alice целевым активом (например через сниппет `register and mint` или потоки quickstart SDK).
- Выполните точку входа `do_transfer`, чтобы перевести 10 единиц от Alice к Bob, удовлетворяя разрешению `AssetTransferRole`.
- Проверьте балансы (`FindAccountAssets`, `iroha_cli ledger assets list`) или подпишитесь на события pipeline, чтобы наблюдать результат перевода.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/transfer-asset.ko)

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
