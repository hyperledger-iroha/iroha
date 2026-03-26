---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e23e082d7c2fdbef72ad5e6b987deaee60ebc457aa57aabeab1f1b3be2c22f93
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/nft-flow
title: הטבעה, העברה ושריפה של NFT
description: מוביל דרך מחזור החיים של NFT מקצה לקצה: הטבעה לבעלים, העברה, תיוג מטא-דאטה ושריפה.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

מוביל דרך מחזור החיים של NFT מקצה לקצה: הטבעה לבעלים, העברה, תיוג מטא-דאטה ושריפה.

## סיור בספר החשבונות

- ודאו שהגדרת ה-NFT (למשל `n0#wonderland`) קיימת לצד חשבונות הבעלים/המקבל המשמשים בסניפט (`soraカタカナ...`, `soraカタカナ...`).
- הפעילו את נקודת הכניסה `nft_issue_and_transfer` כדי להטביע את ה-NFT, להעביר אותו מ-Alice ל-Bob ולצרף דגל מטא-דאטה המתאר את ההנפקה.
- בדקו את מצב ספר ה-NFT באמצעות `iroha_cli ledger nfts list --account <id>` או המקבילות ב-SDK כדי לאמת את ההעברה, ואז ודאו שהנכס מוסר לאחר שהוראת השריפה רצה.

## מדריכי SDK קשורים

- [Quickstart של Rust SDK](/sdks/rust)
- [Quickstart של Python SDK](/sdks/python)
- [Quickstart של JavaScript SDK](/sdks/javascript)

[הורדת מקור Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("soraカタカナ...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("soraカタカナ...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```
