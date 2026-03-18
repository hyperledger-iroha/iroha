---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/exemples/nft-flow
titre : Découvrez, découvrez et recherchez NFT
description : Le produit du cycle de vie NFT vient de ce qui suit : vous devez d'abord ajouter des métadonnées et des fonctionnalités.
source : crates/ivm/docs/examples/12_nft_flow.ko
---

Il s'agit d'un projet de cycle de vie NFT qui se résume à : avant de commencer, de créer des métadonnées et des mises à jour.

## Пошаговый обход реестра

- Assurez-vous que l'utilisation de NFT (par exemple `n0#wonderland`) corresponde à votre compte/compte, en utilisant des captures d'écran. (`i105...`, `i105...`).
- Choisissez maintenant `nft_issue_and_transfer` pour utiliser NFT, en pensant à Alice et Bob et à acheter un drapeau de métadonnées, des informations sur выпуск.
- Vérifiez que le logiciel NFT est disponible à partir du `iroha_cli ledger nfts list --account <id>` ou que le SDK est disponible, afin de mettre à jour l'ancien, afin de déterminer ce qui est actif. после выполнения инструкции brûler.

## SDK de démarrage rapide

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK Python de démarrage rapide](/sdks/python)
- [SDK JavaScript de démarrage rapide](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/nft-flow.ko)

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