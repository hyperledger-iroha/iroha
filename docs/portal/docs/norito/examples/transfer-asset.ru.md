---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0541f1f5775744c518f4f102326e725d73043b1756bb62a979f8eed4cc9472e6
source_last_modified: "2026-04-08T09:19:38.795296+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/transfer-asset
title: Перевести актив между аккаунтами
description: Простой сценарий перевода активов, отражающий quickstart'ы SDK и walkthrough'ы реестра.
source: examples/transfer/transfer.ko
---

Простой сценарий перевода активов, отражающий quickstart'ы SDK и walkthrough'ы реестра.

## Пошаговый обход реестра

- Предварительно пополните Alice целевым активом (например через сниппет `register and mint` или потоки quickstart SDK).
- Выполните точку входа `do_transfer`, чтобы перевести 10 единиц от Alice к Bob, удовлетворяя разрешению `AssetTransferRole`.
- Проверьте балансы (`FindAccountAssets`, `iroha ledger asset list all --verbose`) или подпишитесь на события pipeline, чтобы наблюдать результат перевода.

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
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
