---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/ejemplos/nft-flow
título: سك ونقل وحرق NFT
descripción: يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.
fuente: crates/ivm/docs/examples/12_nft_flow.ko
---

يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.

## جولة دفتر الأستاذ

- تأكد من وجود تعريف NFT (مثل `n0#wonderland`) إلى جانب حسابات المالك/المستلم المستخدمة في المقتطف (`<i105-account-id>`, `<i105-account-id>`).
- Establece el nombre `nft_issue_and_transfer` de NFT y de Alice y Bob y crea una cuenta de usuario.
- Soporte para NFT `iroha_cli ledger nfts list --account <id>` y SDK para dispositivos móviles. تنفيذ تعليمة الحرق.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [Aplicación del SDK de Python](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Actualización Kotodama](/norito-snippets/nft-flow.ko)

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