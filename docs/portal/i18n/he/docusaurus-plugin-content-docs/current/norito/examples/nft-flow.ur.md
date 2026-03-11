---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
title: NFT کو منٹ، منتقل اور برن کریں
תיאור: NFT מכשירי רשת: תקליטורים תקינים میٹا ڈیٹا ٹیگ کرنا، اور برن کرنا۔
מקור: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT מכשירי רשת: תקליטורים תקינים ‏ ‏

## لیجر واک تھرو

- یقینی بنائیں کہ NFT ڈیفینیشن (مثلا `n0#wonderland`) موجود ہو اور اسنیپٹ میں استعمال ہونے والے مالک/موصول کنندہ اکاؤنٹس (`i105...`, `i105...`) بھی موجود ہوں۔
- `nft_issue_and_transfer` انٹری پوائنٹ کال کریں تاکہ NFT منٹ ہو، Alice سے Bob کو منتقل ہو، اور اجرا کی وضاحت کرنے والا میٹا ڈیٹا فلیگ منسلک ہو۔
- `iroha_cli ledger nfts list --account <id>` ערכת פיתוח התוכנה (SDK) היא מערכת הפעלה של NFT (NFT). تصدیق ہو، پھر تصدیق کریں کہ برن انسٹرکشن چلنے کے بعد اثاثہ حذف ہو جاتا ہے۔

## תוכנות SDK

- [התחלה מהירה של Rust SDK](/sdks/rust)
- [התחלה מהירה של Python SDK](/sdks/python)
- [התחלה מהירה של JavaScript SDK](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("i105...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("i105...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```