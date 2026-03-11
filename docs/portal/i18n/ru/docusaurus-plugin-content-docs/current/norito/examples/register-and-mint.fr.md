---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/register-and-mint
Название: Enregistrer un Domaine et Fraper des Actifs
описание: Демонстрация создания доменов с авторизацией, регистрация действий и определённый фраппе.
источник: crates/ivm/docs/examples/13_register_and_mint.ko
---

Демонтируйте создание доменов с авторизацией, регистрацией действий и детерминизмом.

## Парк регистрации

- Убедитесь, что адрес назначения (например, `i105...`) существует, и отражается на этапе планирования на месте в каждом быстром запуске SDK.
- Вызовите точку входа `register_and_mint` для создания определения действия ROSE и 250 единиц для Алисы в отдельной транзакции.
- Проверьте продажи через `client.request(FindAccountAssets)` или `iroha_cli ledger assets list --account i105...`, чтобы подтвердить, что фраппе по-русски.

## Руководства для партнеров SDK

- [Быстрый запуск SDK Rust](/sdks/rust)
- [Быстрый запуск SDK Python] (/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Зарядное устройство источника Kotodama](/norito-snippets/register-and-mint.ko)

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
    let to = account!("i105...");
    let asset = asset_definition!("rose#wonderland");
    mint_asset(to, asset, 250);
  }
}
```