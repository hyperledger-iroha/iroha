---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/exemples/nft-flow
titre : NFT کو منٹ، منتقل اور برن کریں
description : NFT est un produit de référence pour les utilisateurs de NFT : Il s'agit d'un modèle de NFT ڈیٹا ٹیگ کرنا، اور برن کرنا۔
source : crates/ivm/docs/examples/12_nft_flow.ko
---

NFT est un outil de recherche en ligne gratuit : il s'agit d'un modèle de référencement gratuit ڈیٹا ٹیگ کرنا، اور برن کرنا۔

## لیجر واک تھرو

- Le logiciel NFT (`n0#wonderland`) est également disponible en ligne. والے مالک/موصول کنندہ اکاؤنٹس (`<i105-account-id>`, `<i105-account-id>`) par ھی موجود ہوں۔
- `nft_issue_and_transfer` L'application NFT est basée sur Alice et Bob et sur l'application NFT. کرنے والا میٹا ڈیٹا فلیگ منسلک ہو۔
- `iroha_cli ledger nfts list --account <id>` pour un SDK permettant de créer un lien NFT avec un logiciel de stockage ہو، پھر تصدیق کریں کہ برن انسٹرکشن چلنے کے بعد اثاثہ حذف ہو جاتا ہے۔

## Utiliser le SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

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
