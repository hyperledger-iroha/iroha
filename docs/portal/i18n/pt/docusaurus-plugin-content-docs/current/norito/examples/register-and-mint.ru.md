---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/register-and-mint
título: Зарегистрировать домен и выпустить активы
description: Показывает создание доменов с разрешениями, registrate активов и детерминированный выпуск.
fonte: crates/ivm/docs/examples/13_register_and_mint.ko
---

Registre as atividades e determine a sua atividade.

## Пошаговый обход реестра

- Verifique se a conta está configurada (por exemplo, `<i105-account-id>`) para instalar o SDK de início rápido.
- Вызовите точку входа `register_and_mint`, чтобы создать определение актива ROSE e выпустить 250 единиц para Alice em одной транзакции.
- Verifique os saldos de `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account <i105-account-id>`, que podem ser usados.

## Como usar o SDK

- [Início rápido Rust SDK](/sdks/rust)
- [Início rápido do SDK do Python](/sdks/python)
- [SDK JavaScript de início rápido](/sdks/javascript)

[Kotodama](/norito-snippets/register-and-mint.ko)

```kotodama
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
    kotoage fn register_and_mint() authorize("AssetManager") {
        // name, symbol, quantity (precision or supply depending on host), mintable flag
        let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
        let symbol = "ROSE";
        let qty = 1000;
        // interpretation depends on data model (example only)
        let mintable = 1;
        // 1 = mintable, 0 = fixed
        ledger::asset::register(
            asset_definition: asset,
            name: symbol,
            scale: qty,
            mintable: mintable,
        );
        // Mint 250 ROSE to Alice
        let to = AccountId::parse(
            "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
        );
        ledger::asset::mint(account: to, asset_definition: asset, amount: 250);
    }
}
```
