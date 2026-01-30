---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 66ddbf3d1f0fb8f49b26c6ba62ce2a031f272f0ae08b8eb6a480f48ba7e70e4e
source_last_modified: "2025-11-14T04:43:20.791896+00:00"
translation_last_reviewed: 2026-01-30
---

يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.

## جولة دفتر الأستاذ

- تأكد من وجود حساب الوجهة (مثل `ih58...`) بما يعكس مرحلة الإعداد في كل بدء سريع للـ SDK.
- استدعِ نقطة الدخول `register_and_mint` لإنشاء تعريف أصل ROSE وسك 250 وحدة لأليس في معاملة واحدة.
- تحقق من الأرصدة عبر `client.request(FindAccountAssets)` أو `iroha_cli ledger assets list --account ih58...` لتأكيد نجاح السك.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

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
    let to = account!("ih58...");
    let asset = asset_definition!("rose#wonderland");
    mint_asset(to, asset, 250);
  }
}
```
