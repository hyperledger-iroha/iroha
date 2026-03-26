---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/register-and-mint
Название: تسجيل نطاق وسك الأصول
Описание: Он играет в фильме "Панель Уэйд" в роли убийцы.
источник: crates/ivm/docs/examples/13_register_and_mint.ko
---

Он был убит в 2007 году в Нью-Йорке.

## جولة دفتر الأستاذ

- Написано в фильме "Проект" (в роли `i105...`) Загрузите SDK.
- استدعِ نقطة الدخول `register_and_mint` لإنشاء تعريف أصل ROSE وسك 250 وحدة لأليس في معاملة واحدة.
- Вы можете установить `client.request(FindAccountAssets)` или `iroha_cli ledger assets list --account i105...` в исходное состояние.

## Использование SDK

- [Загрузка в Rust SDK](/sdks/rust)
- [Просмотр Python SDK](/sdks/python)
- [Загрузка JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/register-and-mint.ko)

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