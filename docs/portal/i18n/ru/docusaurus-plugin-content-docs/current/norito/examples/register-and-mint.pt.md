---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/register-and-mint
титул: Регистратор dominio e cunhar ativos
описание: Демонстрация разрешения на владение имуществом, регистрация активных действий и детерминированный доступ.
источник: crates/ivm/docs/examples/13_register_and_mint.ko
---

Продемонстрируйте создание доменов с разрешениями, регистрацию активных действий и детерминированный доступ.

## Ротейру до Ливро Разау

- Гарантия, что предназначение (например, `i105...`) существует, необходимо выполнить этап настройки в каждом кратком руководстве по SDK.
- Вызовите точку входа `register_and_mint`, чтобы вызвать определенное действие ROSE и выполнить 250 операций для Алисы в уникальной транзакции.
- Проверьте свои данные через `client.request(FindAccountAssets)` или `iroha_cli ledger assets list --account i105...`, чтобы подтвердить, что это удалось сделать.

## Рекомендации по использованию SDK

- [Краткий старт работы с SDK Rust](/sdks/rust)
- [Краткий старт работы с SDK Python](/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Вставьте шрифт Kotodama](/norito-snippets/register-and-mint.ko)

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
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```