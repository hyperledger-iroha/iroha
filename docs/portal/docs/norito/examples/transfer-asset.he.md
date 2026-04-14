---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0541f1f5775744c518f4f102326e725d73043b1756bb62a979f8eed4cc9472e6
source_last_modified: "2026-04-08T09:19:38.795296+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/transfer-asset
title: העברת נכס בין חשבונות
description: תהליך העברה פשוט של נכסים שמשקף את ה-quickstarts של ה-SDK ואת סיורי ספר החשבונות.
source: examples/transfer/transfer.ko
---

תהליך העברה פשוט של נכסים שמשקף את ה-quickstarts של ה-SDK ואת סיורי ספר החשבונות.

## סיור בספר החשבונות

- ממן את Alice בנכס היעד מראש (לדוגמה דרך הסניפט `register and mint` או הזרימות של quickstart SDK).
- הפעילו את נקודת הכניסה `do_transfer` כדי להעביר 10 יחידות מ-Alice ל-Bob, תוך עמידה בהרשאת `AssetTransferRole`.
- בדקו יתרות (`FindAccountAssets`, `iroha ledger asset list all --verbose`) או הירשמו לאירועי pipeline כדי לראות את תוצאת ההעברה.

## מדריכי SDK קשורים

- [Quickstart של Rust SDK](/sdks/rust)
- [Quickstart של Python SDK](/sdks/python)
- [Quickstart של JavaScript SDK](/sdks/javascript)

[הורדת מקור Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
