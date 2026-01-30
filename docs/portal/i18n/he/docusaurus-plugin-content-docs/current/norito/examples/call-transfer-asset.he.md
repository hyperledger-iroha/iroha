---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: db5ff695a18716703a79fe66243cdfcde9d6e1f2e9bbd8d7842222df6a10f0a8
source_last_modified: "2025-11-14T04:43:20.644414+00:00"
translation_last_reviewed: 2026-01-30
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
