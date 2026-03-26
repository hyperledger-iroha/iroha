---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/exemples/nft-flow
titre : Acuñar, transférer et utiliser un NFT
description : Enregistrez le cycle de vie d'un NFT de l'extrême à l'extrême : connaissance du propriétaire, transfert, étiquette des métadonnées et des questions.
source : crates/ivm/docs/examples/12_nft_flow.ko
---

Enregistrez le cycle de vie d'un NFT de l'extrême à l'extrême : connaissance du propriétaire, transfert, étiquette des métadonnées et des questions.

## Recorrido del libro mayor

- Assurez-vous que la définition du NFT existe (par exemple `n0#wonderland`) avec les données du propriétaire/récepteur utilisées dans le fragment (`<katakana-i105-account-id>`, `<katakana-i105-account-id>`).
- Appelez le point d'entrée `nft_issue_and_transfer` pour connaître le NFT, transférer Alice à Bob et ajouter une bandera de métadonnées qui décrit l'émission.
- Inspectez l'état du livre principal de NFT avec `iroha_cli ledger nfts list --account <id>` ou les équivalents du SDK pour vérifier le transfert, puis confirmez que l'actif est éliminé une fois que l'instruction est exécutée.

## Guides relatifs au SDK

- [Démarrage rapide du SDK de Rust](/sdks/rust)
- [Démarrage rapide du SDK de Python](/sdks/python)
- [Démarrage rapide du SDK de JavaScript](/sdks/javascript)

[Télécharger la source de Kotodama](/norito-snippets/nft-flow.ko)

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