---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 470eb5de8cd9a7f94275062d1e8c3a448a2d734bf86f650ce94a3971baa3527d
source_last_modified: "2026-04-08T09:19:38.793794+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/register-and-mint
title: تسجيل نطاق وسك الأصول
description: يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.

## جولة دفتر الأستاذ

- تأكد من وجود حساب الوجهة (مثل `<i105-account-id>`) بما يعكس مرحلة الإعداد في كل بدء سريع للـ SDK.
- استدعِ نقطة الدخول `register_and_mint` لإنشاء تعريف أصل ROSE وسك 250 وحدة لأليس في معاملة واحدة.
- تحقق من الأرصدة عبر `client.request(FindAccountAssets)` أو `iroha ledger asset list all --verbose` لتأكيد نجاح السك.

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
    let to = account!("<i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
