---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/register-and-mint
título: Зарегистрировать domен и выпустить активы
descripción: Показывает создание доменов с разрешениями, регистрацию активов and детерминированный выпуск.
fuente: crates/ivm/docs/examples/13_register_and_mint.ko
---

Conecte los hogares con la configuración, el registro de activos y los controles predeterminados.

## Пошаговый обход реестра

- Tenga en cuenta que esta cuenta de usuario (principal `soraカタカナ...`) está almacenada en el SDK de inicio rápido.
- Utilice este vídeo `register_and_mint`, mantenga activada la activa ROSE y escriba 250 ediciones de Alice en una sola transmisión.
- Asegúrese de que los equilibrios sean `client.request(FindAccountAssets)` o `iroha_cli ledger assets list --account soraカタカナ...`, para poder utilizar datos personales.

## Связанные руководства SDK

- [SDK de inicio rápido de Rust](/sdks/rust)
- [SDK de Python de inicio rápido](/sdks/python)
- [SDK de JavaScript de inicio rápido](/sdks/javascript)

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
    let to = account!("soraカタカナ...");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```