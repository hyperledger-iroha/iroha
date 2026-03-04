---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/این ایف ٹی فلو
عنوان: ٹکسال ، منتقلی اور برن این ایف ٹی
تفصیل: شروع سے ختم ہونے تک کسی این ایف ٹی کے لائف سائیکل کی فہرست: مالک کے لئے ٹکسال ، منتقلی ، میٹا ڈیٹا ٹیگنگ اور جلانے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/12_NFT_FLOW.KO
---

اس میں شروع سے ختم ہونے تک کسی این ایف ٹی کے لائف سائیکل کی فہرست دی گئی ہے: مالک کے لئے ٹکسال کرنا ، منتقلی ، میٹا ڈیٹا ٹیگنگ ، اور جلانے۔

## لیجر ٹور

- اس بات کو یقینی بنائیں کہ این ایف ٹی کی شناخت (جیسے `n0#wonderland`) اسنیپٹ (`ih58...` ، `ih58...`) میں استعمال ہونے والے مالک/وصول کنندہ اکاؤنٹس کے ساتھ موجود ہے۔
- کال انٹری پوائنٹ `nft_issue_and_transfer` کو این ایف ٹی کی پودینہ پر ، ایلس سے باب میں منتقل کریں ، اور ورژن کی وضاحت کرنے والے میٹا ڈیٹا ٹیگ کو منسلک کریں۔
- منتقلی کی تصدیق کے ل I `iroha_cli ledger nfts list --account <id>` یا SDK کے مساویوں کا استعمال کرتے ہوئے NFT کتاب کی حیثیت کو چیک کریں ، پھر برن ہدایت پر عمل درآمد کے بعد اثاثہ کو ہٹانے کی تصدیق کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر SDK کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("ih58...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("ih58...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```