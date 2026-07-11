---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/register-and-mint
titre : ڈومین رجسٹر کریں اور اثاثے منٹ کریں
description: اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔
source : crates/ivm/docs/examples/13_register_and_mint.ko
---

اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔

## لیجر واک تھرو

- Téléchargez le guide de démarrage rapide du SDK (`<i105-account-id>`) pour démarrer rapidement le SDK. عکاسی کرتا ہے۔
- `register_and_mint` انٹری پوائنٹ کال کریں تاکہ ROSE اثاثہ ڈیفینیشن بنے اور ایک ہی ٹرانزیکشن Alice a 250 ans d'argent
- `client.request(FindAccountAssets)` et `iroha_cli ledger assets list --account <i105-account-id>` sont en cours de mise à jour en cas de problème. ہو۔

## Utiliser le SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/register-and-mint.ko)

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
