---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/call-transfer-asset
title: הפעלת העברה מהמארח מתוך Kotodama
description: מדגים כיצד נקודת כניסה של Kotodama יכולה לקרוא להוראת המארח `transfer_asset` עם אימות מטא-דאטה מקוון.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

מדגים כיצד נקודת כניסה של Kotodama יכולה לקרוא להוראת המארח `transfer_asset` עם אימות מטא-דאטה מקוון.

## סיור בספר החשבונות

- ממן את סמכות החוזה (למשל `soraカタカナ...`) בנכס שהיא תעביר והעניקו לסמכות את תפקיד `CanTransfer` או הרשאה שקולה.
- קראו לנקודת הכניסה `call_transfer_asset` כדי להעביר 5 יחידות מחשבון החוזה אל `soraカタカナ...`, באופן שמשקף כיצד אוטומציה על השרשרת יכולה לעטוף קריאות מארח.
- אמתו יתרות דרך `FindAccountAssets` או `iroha_cli ledger assets list --account soraカタカナ...` ובדקו אירועים כדי לאשר ששומר המטא-דאטה רשם את הקשר ההעברה.

## מדריכי SDK קשורים

- [Quickstart של Rust SDK](/sdks/rust)
- [Quickstart של Python SDK](/sdks/python)
- [Quickstart של JavaScript SDK](/sdks/javascript)

[הורדת מקור Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
