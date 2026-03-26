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

- Le logiciel NFT (`n0#wonderland`) est également disponible en ligne. والے مالک/موصول کنندہ اکاؤنٹس (`<katakana-i105-account-id>`, `<katakana-i105-account-id>`) par ھی موجود ہوں۔
- `nft_issue_and_transfer` L'application NFT est basée sur Alice et Bob et sur l'application NFT. کرنے والا میٹا ڈیٹا فلیگ منسلک ہو۔
- `iroha_cli ledger nfts list --account <id>` pour un SDK permettant de créer un lien NFT avec un logiciel de stockage ہو، پھر تصدیق کریں کہ برن انسٹرکشن چلنے کے بعد اثاثہ حذف ہو جاتا ہے۔

## Utiliser le SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/nft-flow.ko)

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