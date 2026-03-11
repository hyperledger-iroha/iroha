---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/این ایف ٹی فلو
عنوان: ٹکسال ، منتقلی اور جلانے والا این ایف ٹی
تفصیل: این ایف ٹی کے زندگی سے لے کر اختتام سے آخر تک چلیں: مالک کو ٹکسال کرنا ، منتقلی ، میٹا ڈیٹا ٹیگنگ اور جلانا۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/12_NFT_FLOW.KO
---

یہ این ایف ٹی کے زندگی سے لے کر اختتام تک جاتا ہے: مالک کو ٹکسال کرنا ، منتقلی ، میٹا ڈیٹا ٹیگنگ اور جلانا۔

## لیجر ٹور

- اس بات کو یقینی بنائیں کہ NFT کی تعریف موجود ہے (مثال کے طور پر `n0#wonderland`) کے ساتھ ساتھ شارڈ (`i105...` ، `i105...`) میں استعمال ہونے والے مالک/وصول کنندہ اکاؤنٹس کے ساتھ۔
- این ایف ٹی کو ٹکسال کرنے کے لئے انٹری پوائنٹ `nft_issue_and_transfer` ، ایلس سے باب میں منتقل کریں ، اور اجراء کو بیان کرنے والے میٹا ڈیٹا پرچم منسلک کریں۔
- منتقلی کی تصدیق کے ل I `iroha_cli ledger nfts list --account <id>` یا SDK کے مساویوں کے ساتھ NFT لیجر کی حیثیت کا معائنہ کرتا ہے ، پھر اس بات کی تصدیق کرتا ہے کہ جلنے کی ہدایت پر عمل درآمد ہونے کے بعد اثاثہ حذف ہوجاتا ہے۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[Kotodama کا ماخذ ڈاؤن لوڈ کریں] (/norito-snippets/nft-flow.ko)

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