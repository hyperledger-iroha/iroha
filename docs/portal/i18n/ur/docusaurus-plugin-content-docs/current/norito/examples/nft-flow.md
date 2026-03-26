---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/nft-flow
title: NFT کو منٹ، منتقل اور برن کریں
description: NFT کے لائف سائیکل کو ابتدا سے انتہا تک دکھاتا ہے: مالک کو منٹ کرنا، منتقل کرنا، میٹا ڈیٹا ٹیگ کرنا، اور برن کرنا۔
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT کے لائف سائیکل کو ابتدا سے انتہا تک دکھاتا ہے: مالک کو منٹ کرنا، منتقل کرنا، میٹا ڈیٹا ٹیگ کرنا، اور برن کرنا۔

## لیجر واک تھرو

- یقینی بنائیں کہ NFT ڈیفینیشن (مثلا `n0#wonderland`) موجود ہو اور اسنیپٹ میں استعمال ہونے والے مالک/موصول کنندہ اکاؤنٹس (`soraカタカナ...`, `soraカタカナ...`) بھی موجود ہوں۔
- `nft_issue_and_transfer` انٹری پوائنٹ کال کریں تاکہ NFT منٹ ہو، Alice سے Bob کو منتقل ہو، اور اجرا کی وضاحت کرنے والا میٹا ڈیٹا فلیگ منسلک ہو۔
- `iroha_cli ledger nfts list --account <id>` یا SDK کے متبادل استعمال کر کے NFT لیجر اسٹیٹ دیکھیں تاکہ ٹرانسفر کی تصدیق ہو، پھر تصدیق کریں کہ برن انسٹرکشن چلنے کے بعد اثاثہ حذف ہو جاتا ہے۔

## متعلقہ SDK گائیڈز

- [Rust SDK quickstart](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/nft-flow.ko)

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
