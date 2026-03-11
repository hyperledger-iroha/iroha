---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラグ: /norito/examples/nft-flow
タイトル: NFT と منٹ، منتقل اور برن کریں
説明: NFT لائف سائیکل کو ابتدا سے انتہا تک دکھاتا ہے: مالک کو منٹ کرنا، منتقل کرنا، میٹا ٹیٹا ٹیگ کرنا، اور برن کرنا۔
ソース: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT の評価: مالک کو منٹ کرنا، منتقل کرنا، میٹا ٹیٹا ٹیگ کرنا، اور برن کرنا۔

## ٩جر واک تھرو

- یقینی بنائیں کہ NFT ڈیفینیشن (مثلا `n0#wonderland`) موجود ہو اور اسنیپٹ میں استعمال ہونے والے مالک/موصول کنندہ اکاؤنٹس (`i105...`, `i105...`) بھی موجود ہوں۔
- `nft_issue_and_transfer` انٹری پوائنٹ کال کریں تاکہ NFT منٹ ہو، アリス سے ボブ کو منتقل ہو، اور اجرا کی وضاحت ٩رنے والا میٹا ڈیٹا فلیگ منسلک ہو۔
- `iroha_cli ledger nfts list --account <id>` SDK セキュリティ セキュリティ NFT セキュリティ セキュリティ SDK セキュリティ セキュリティ NFT セキュリティ セキュリティتصدیق ہو، پھر تصدیق کریں کہ برن انسٹرکشن چلنے کے بعد اثاثہ حذف ہو جاتا ہے۔

## SDK の開発

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

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