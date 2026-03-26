---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/exemples/nft-flow
titre : سك ونقل وحرق NFT
description: يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.
source : crates/ivm/docs/examples/12_nft_flow.ko
---

Il s'agit d'un NFT proche de la réalité : il s'agit d'une source de revenus et d'une source d'informations.

## جولة دفتر الأستاذ

- Il s'agit d'un NFT (`n0#wonderland`) ou d'un lien vers un service client. (`<i105-account-id>`, `<i105-account-id>`).
- Il s'agit d'un `nft_issue_and_transfer` pour NFT et d'Alice et Bob et d'un proche.
- Utilisez le logiciel NFT `iroha_cli ledger nfts list --account <id>` pour utiliser le SDK pour votre recherche. تنفيذ تعليمة الحرق.

## Le SDK est disponible

- [Détails du SDK Rust](/sdks/rust)
- [Détails du SDK Python](/sdks/python)
- [Détails du SDK JavaScript](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("<i105-account-id>");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("<i105-account-id>");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```