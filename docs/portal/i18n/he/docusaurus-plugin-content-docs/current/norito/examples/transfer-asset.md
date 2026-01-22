<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/transfer-asset
title: העברת נכס בין חשבונות
description: תהליך העברה פשוט של נכסים שמשקף את ה-quickstarts של ה-SDK ואת סיורי ספר החשבונות.
source: examples/transfer/transfer.ko
---

תהליך העברה פשוט של נכסים שמשקף את ה-quickstarts של ה-SDK ואת סיורי ספר החשבונות.

## סיור בספר החשבונות

- ממן את Alice בנכס היעד מראש (לדוגמה דרך הסניפט `register and mint` או הזרימות של quickstart SDK).
- הפעילו את נקודת הכניסה `do_transfer` כדי להעביר 10 יחידות מ-Alice ל-Bob, תוך עמידה בהרשאת `AssetTransferRole`.
- בדקו יתרות (`FindAccountAssets`, `iroha_cli assets list`) או הירשמו לאירועי pipeline כדי לראות את תוצאת ההעברה.

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
