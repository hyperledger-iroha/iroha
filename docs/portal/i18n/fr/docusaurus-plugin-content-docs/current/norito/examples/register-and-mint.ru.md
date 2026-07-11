---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/register-and-mint
title: Enregistrer la maison et activer les activités
description: Показывает создание доменов с разрешениями, регистрацию активов и детерминированный выпуск.
source : crates/ivm/docs/examples/13_register_and_mint.ko
---

Показывает создание доменов с разрешениями, регистрацию активов и детерминированный выпуск.

## Пошаговый обход реестра

- Assurez-vous que le compte de démarrage (par exemple `<i105-account-id>`) soit disponible à l'étape suivante du SDK de démarrage rapide.
- Vous avez choisi `register_and_mint` pour pouvoir exploiter l'action ROSE et gagner 250 éditions pour Alice dans une nouvelle transition.
- Vérifiez les soldes correspondant à `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account <i105-account-id>` pour pouvoir modifier votre compte.

## SDK de démarrage rapide

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK Python de démarrage rapide](/sdks/python)
- [SDK JavaScript de démarrage rapide](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/register-and-mint.ko)

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
        ledger::asset::mint(account: to, asset_definition: asset, amount: Amount::from_i64(250));
    }
}
```
