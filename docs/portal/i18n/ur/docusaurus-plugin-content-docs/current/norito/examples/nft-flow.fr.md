---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/این ایف ٹی فلو
عنوان: ٹکسال ، منتقلی اور جلانے والا این ایف ٹی
تفصیل: این ایف ٹی کے زندگی سے لے کر آخر تک اختتام تک چلتا ہے: مالک کو ٹکسال کرنا ، منتقلی ، میٹا ڈیٹا شامل کرنا ، اور تباہی۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/12_NFT_FLOW.KO
---

کسی این ایف ٹی کے زندگی سے لے کر آخر سے آخر تک چلتا ہے: مالک کو ٹکسال کرنا ، منتقلی ، میٹا ڈیٹا شامل کرنا ، اور تباہی۔

## رجسٹری براؤزنگ

- اس بات کو یقینی بنائیں کہ NFT تعریف (جیسے `n0#wonderland`) اسنیپٹ (`<katakana-i105-account-id>` ، `<katakana-i105-account-id>`) میں استعمال ہونے والے مالک/وصول کنندہ اکاؤنٹس کے ساتھ موجود ہے۔
- انٹری پوائنٹ `nft_issue_and_transfer` کو این ایف ٹی کو ٹکسال کرنے کے لئے ، اسے ایلس سے باب میں منتقل کریں ، اور اجراء کو بیان کرنے والے میٹا ڈیٹا پرچم منسلک کریں۔
- منتقلی کی تصدیق کے ل I `iroha_cli ledger nfts list --account <id>` یا SDK کے مساویوں کے ساتھ NFT لیجر کی حالت کا معائنہ کریں ، پھر اس بات کی تصدیق کریں کہ جلنے کی ہدایت پر عمل درآمد ہونے کے بعد اثاثہ حذف ہوجاتا ہے۔

## متعلقہ SDK گائیڈز

- [کوئک اسٹارٹ SDK مورچا] (/sdks/rust)
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