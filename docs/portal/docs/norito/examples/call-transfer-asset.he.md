---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a91fc8841580a836c80129942df7f79f5bc5dd5f6a72dccf1394b740d02536a5
source_last_modified: "2025-11-23T15:30:33.687233+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /norito/examples/call-transfer-asset
title: הפעלת העברה מהמארח מתוך Kotodama
description: מדגים כיצד נקודת כניסה של Kotodama יכולה לקרוא להוראת המארח `transfer_asset` עם אימות מטא-דאטה מקוון.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

מדגים כיצד נקודת כניסה של Kotodama יכולה לקרוא להוראת המארח `transfer_asset` עם אימות מטא-דאטה מקוון.

## סיור בספר החשבונות

- ממן את סמכות החוזה (למשל `ih58...`) בנכס שהיא תעביר והעניקו לסמכות את תפקיד `CanTransfer` או הרשאה שקולה.
- קראו לנקודת הכניסה `call_transfer_asset` כדי להעביר 5 יחידות מחשבון החוזה אל `ih58...`, באופן שמשקף כיצד אוטומציה על השרשרת יכולה לעטוף קריאות מארח.
- אמתו יתרות דרך `FindAccountAssets` או `iroha_cli ledger assets list --account ih58...` ובדקו אירועים כדי לאשר ששומר המטא-דאטה רשם את הקשר ההעברה.

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
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
