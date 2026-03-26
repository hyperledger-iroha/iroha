---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c30c710be94cd99f3c7a0484040155bf63ff4dc0d464d76237bddc8bf589ef26
source_last_modified: "2025-11-07T11:59:47.168250+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /norito/examples/register-and-mint
title: Зарегистрировать домен и выпустить активы
description: Показывает создание доменов с разрешениями, регистрацию активов и детерминированный выпуск.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

Показывает создание доменов с разрешениями, регистрацию активов и детерминированный выпуск.

## Пошаговый обход реестра

- Убедитесь, что аккаунт назначения (например `<katakana-i105-account-id>`) существует, повторяя фазу подготовки в каждом quickstart SDK.
- Вызовите точку входа `register_and_mint`, чтобы создать определение актива ROSE и выпустить 250 единиц для Alice в одной транзакции.
- Проверьте балансы через `client.request(FindAccountAssets)` или `iroha_cli ledger assets list --account <katakana-i105-account-id>`, чтобы подтвердить успешный выпуск.

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
    let to = account!("<katakana-i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
