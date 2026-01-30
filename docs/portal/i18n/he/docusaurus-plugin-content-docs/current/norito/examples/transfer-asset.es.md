---
lang: es
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1323ab63bee56af163635cbe5c1d25e09baaaf0eb3e2afaeaa9e243ce3b924fc
source_last_modified: "2025-11-04T12:26:02.943204+00:00"
translation_last_reviewed: 2026-01-30
---

תהליך העברה פשוט של נכסים שמשקף את ה-quickstarts של ה-SDK ואת סיורי ספר החשבונות.

## סיור בספר החשבונות

- ממן את Alice בנכס היעד מראש (לדוגמה דרך הסניפט `register and mint` או הזרימות של quickstart SDK).
- הפעילו את נקודת הכניסה `do_transfer` כדי להעביר 10 יחידות מ-Alice ל-Bob, תוך עמידה בהרשאת `AssetTransferRole`.
- בדקו יתרות (`FindAccountAssets`, `iroha_cli ledger assets list`) או הירשמו לאירועי pipeline כדי לראות את תוצאת ההעברה.

## מדריכי SDK קשורים

- [Quickstart של Rust SDK](/sdks/rust)
- [Quickstart של Python SDK](/sdks/python)
- [Quickstart של JavaScript SDK](/sdks/javascript)

[הורדת מקור Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
