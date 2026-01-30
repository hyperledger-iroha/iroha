---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
slug: /norito/examples/register-and-mint
title: Зарегистрировать домен и выпустить активы
description: Показывает создание доменов с разрешениями, регистрацию активов и детерминированный выпуск.
source: crates/ivm/docs/examples/13_register_and_mint.ko
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
