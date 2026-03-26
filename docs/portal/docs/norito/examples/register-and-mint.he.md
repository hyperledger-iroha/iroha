---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c30c710be94cd99f3c7a0484040155bf63ff4dc0d464d76237bddc8bf589ef26
source_last_modified: "2025-11-07T11:59:47.168250+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /norito/examples/register-and-mint
title: רישום דומיין והטבעת נכסים
description: מדגים יצירת דומיינים עם הרשאה, רישום נכסים והטבעה דטרמיניסטית.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

מדגים יצירת דומיינים עם הרשאה, רישום נכסים והטבעה דטרמיניסטית.

## סיור בספר החשבונות

- ודאו שחשבון היעד (לדוגמה `<katakana-i105-account-id>`) קיים, בדומה לשלב ההכנה בכל quickstart של ה-SDK.
- הפעילו את נקודת הכניסה `register_and_mint` כדי ליצור את הגדרת הנכס ROSE ולהטביע 250 יחידות עבור Alice בעסקה אחת.
- אמתו יתרות דרך `client.request(FindAccountAssets)` או `iroha_cli ledger assets list --account <katakana-i105-account-id>` כדי לוודא שההטבעה הצליחה.

## מדריכי SDK קשורים

- [Quickstart של Rust SDK](/sdks/rust)
- [Quickstart של Python SDK](/sdks/python)
- [Quickstart של JavaScript SDK](/sdks/javascript)

[הורדת מקור Kotodama](/norito-snippets/register-and-mint.ko)

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
    let to = account!("<katakana-i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
