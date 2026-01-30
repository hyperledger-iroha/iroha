---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50ca8178960700e4f999c5d921f1e2b1e36c9c6f26e2de93d89f3b96006354e7
source_last_modified: "2025-11-14T04:43:20.791684+00:00"
translation_last_reviewed: 2026-01-30
---

Показывает создание доменов с разрешениями, регистрацию активов и детерминированный выпуск.

## Пошаговый обход реестра

- Убедитесь, что аккаунт назначения (например `ih58...`) существует, повторяя фазу подготовки в каждом quickstart SDK.
- Вызовите точку входа `register_and_mint`, чтобы создать определение актива ROSE и выпустить 250 единиц для Alice в одной транзакции.
- Проверьте балансы через `client.request(FindAccountAssets)` или `iroha_cli ledger assets list --account ih58...`, чтобы подтвердить успешный выпуск.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("ih58...");
    let asset = asset_definition!("rose#wonderland");
    mint_asset(to, asset, 250);
  }
}
```
