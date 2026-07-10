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

- یقینی بنائیں کہ NFT ڈیفینیشن (مثلا `n0#wonderland`) موجود ہو اور اسنیپٹ میں استعمال ہونے والے مالک/موصول کنندہ اکاؤنٹس (`<i105-account-id>`, `<i105-account-id>`) بھی موجود ہوں۔
- `nft_issue_and_transfer` انٹری پوائنٹ کال کریں تاکہ NFT منٹ ہو، アリス سے ボブ کو منتقل ہو، اور اجرا کی وضاحت ٩رنے والا میٹا ڈیٹا فلیگ منسلک ہو۔
- `iroha_cli ledger nfts list --account <id>` SDK セキュリティ セキュリティ NFT セキュリティ セキュリティ SDK セキュリティ セキュリティ NFT セキュリティ セキュリティتصدیق ہو، پھر تصدیق کریں کہ برن انسٹرکشن چلنے کے بعد اثاثہ حذف ہو جاتا ہے۔

## SDK の開発

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76");
        ledger::nft::transfer(owner, nft, to);
        ledger::nft::set_metadata(nft, Name::parse("issued"), Json::parse("{\"issued\":\"demo\"}"));
        ledger::nft::burn(nft);
    }
}
```
