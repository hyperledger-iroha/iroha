---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/register-and-mint
titre : تسجيل نطاق وسك الأصول
description: يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.
source : crates/ivm/docs/examples/13_register_and_mint.ko
---

يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.

## جولة دفتر الأستاذ

- Utilisez le SDK (`<i105-account-id>`) pour télécharger le SDK.
- Le prix du produit `register_and_mint` est de 250 $ pour ROSE.
- تحقق من أرصدة عبر `client.request(FindAccountAssets)` et `iroha_cli ledger assets list --account <i105-account-id>` لتأكيد نجاح السك.

## Le SDK est disponible

- [Détails du SDK Rust](/sdks/rust)
- [Détails du SDK Python](/sdks/python)
- [Détails du SDK JavaScript](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/register-and-mint.ko)

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
        let to = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", );
        ledger::asset::mint(account: to, asset_definition: asset, amount: 250);
    }
}
```
