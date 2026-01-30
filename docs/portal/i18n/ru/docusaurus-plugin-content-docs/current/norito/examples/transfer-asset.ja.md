---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d0f7a51747942db8f41f79fde18bca64e9605f9786bf49f26af4a2fdb6c9ec9
source_last_modified: "2025-11-14T04:43:20.812331+00:00"
translation_last_reviewed: 2026-01-30
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
