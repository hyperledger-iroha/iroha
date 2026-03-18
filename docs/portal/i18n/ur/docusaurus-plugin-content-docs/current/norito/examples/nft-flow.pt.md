---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/این ایف ٹی فلو
عنوان: ٹکسال ، منتقلی اور جلانے والا این ایف ٹی
تفصیل: شروع سے ختم ہونے تک کسی این ایف ٹی کے لائف سائیکل سے گزرتا ہے: مالک کے لئے ٹکسال کرنا ، منتقلی ، میٹا ڈیٹا ٹیگنگ اور جلانا۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/12_NFT_FLOW.KO
---

شروع سے ختم ہونے تک کسی این ایف ٹی کے لائف سائیکل سے گزرتا ہے: مالک کو ٹکسال کرنا ، منتقلی ، میٹا ڈیٹا ٹیگنگ ، اور جلا دینا۔

## لیجر اسکرپٹ

- اس بات کو یقینی بنائیں کہ این ایف ٹی تعریف (جیسے `n0#wonderland`) اسنیپٹ (`i105...` ، `i105...`) میں استعمال ہونے والے مالک/وصول کنندہ اکاؤنٹس کے ساتھ موجود ہے۔
- `nft_issue_and_transfer` انٹری پوائنٹ کو این ایف ٹی کو ٹکسال کرنے کے لئے طلب کریں ، اسے ایلس سے باب میں منتقل کریں ، اور میٹا ڈیٹا سگنل منسلک کریں جو جاری کرنے کی وضاحت کرتا ہے۔
- منتقلی کی تصدیق کے ل I `iroha_cli ledger nfts list --account <id>` یا SDK کے مساویوں کے ساتھ NFT لیجر کی حالت کا معائنہ کریں ، پھر اس بات کی تصدیق کریں کہ جب برن کی ہدایت چلتی ہے تو اثاثہ ہٹا دیا جاتا ہے۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/nft-flow.ko)

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