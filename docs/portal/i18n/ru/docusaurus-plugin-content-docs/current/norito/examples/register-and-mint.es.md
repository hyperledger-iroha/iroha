---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/register-and-mint
титул: Регистратор доменов и активных действий
описание: Demuestra la creación de dominios con Permisos, el registero de activos y la acuñación determinista.
источник: crates/ivm/docs/examples/13_register_and_mint.ko
---

Продемонстрируйте создание прав собственности с разрешениями, реестр действий и детерминированный учет.

## Запись мэра библиотеки

- Убедитесь, что существующий адрес назначения (например, `soraカタカナ...`), отобразите этап настройки в каждом быстром запуске SDK.
- Вызовите точку входа `register_and_mint`, чтобы создать определение активности ROSE и добавить 250 единиц для Алисы в одной транзакции.
- Проверьте балансы через `client.request(FindAccountAssets)` или `iroha_cli ledger assets list --account soraカタカナ...`, чтобы подтвердить, что ваша запись вышла.

## Руководство по настройке SDK

- [Краткий запуск SDK Rust](/sdks/rust)
- [Краткий запуск SDK Python] (/sdks/python)
- [Краткий запуск SDK JavaScript](/sdks/javascript)

[Удалить ссылку Kotodama](/norito-snippets/register-and-mint.ko)

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
    let to = account!("soraカタカナ...");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```