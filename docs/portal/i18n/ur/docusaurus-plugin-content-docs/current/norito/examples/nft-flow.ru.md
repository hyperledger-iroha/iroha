---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/این ایف ٹی فلو
عنوان: ریلیز ، منتقلی اور برن این ایف ٹی ایس
تفصیل: شروع سے ختم ہونے تک کسی این ایف ٹی کے زندگی کے چکر میں آپ کو چلتا ہے: مالک پر رہائی ، ترجمہ ، میٹا ڈیٹا شامل کرنا ، اور جلانا۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/12_NFT_FLOW.KO
---

شروع سے ختم ہونے تک کسی این ایف ٹی کے زندگی کے چکر سے گزرتا ہے: مالک کو جاری کرنا ، ترجمہ ، میٹا ڈیٹا شامل کرنا ، اور جلانا۔

## قدم بہ قدم رجسٹری ٹریورسل

- اس بات کو یقینی بنائیں کہ این ایف ٹی تعریف (مثال کے طور پر `n0#wonderland`) اسنیپٹ (`<katakana-i105-account-id>` ، `<katakana-i105-account-id>`) میں استعمال ہونے والے مالک/وصول کنندہ اکاؤنٹس کے ساتھ موجود ہے۔
- کال انٹری پوائنٹ `nft_issue_and_transfer` NFT کو جاری کرنے کے لئے ، اسے ایلس سے باب میں منتقل کریں ، اور رہائی کی وضاحت کرنے والے میٹا ڈیٹا پرچم منسلک کریں۔
- منتقلی کی تصدیق کے ل I `iroha_cli ledger nfts list --account <id>` یا SDK کے مساویوں کے ذریعے NFT رجسٹری کی حیثیت کو چیک کریں ، پھر اس بات کو یقینی بنائیں کہ برن ہدایت پر عمل درآمد کے بعد اثاثہ حذف ہو گیا ہے۔

## متعلقہ SDK سبق

- [کوئیک اسٹارٹ مورچا SDK] (/sdks/rust)
- [کوئک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("<katakana-i105-account-id>");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("<katakana-i105-account-id>");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```