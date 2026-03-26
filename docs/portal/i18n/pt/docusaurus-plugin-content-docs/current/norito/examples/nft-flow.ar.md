---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
título: سك ونقل وحرق NFT
description: يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك, النقل, ووسم بيانات التعريف, والحرق.
fonte: crates/ivm/docs/examples/12_nft_flow.ko
---

يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك, النقل, ووسم بيانات التعريف, والحرق.

## جولة دفتر الأستاذ

- تأكد من وجود تعريف NFT (مثل `n0#wonderland`) إلى جانب حسابات المالك/المستلم المستخدمة في المقتطف (`<katakana-i105-account-id>`, `<katakana-i105-account-id>`).
- استدعِ نقطة الدخول `nft_issue_and_transfer` para NFT e من Alice إلى Bob وإرفاق علامة بيانات تعريف تصف الإصدار.
- Faça o download do NFT `iroha_cli ledger nfts list --account <id>` ou do SDK para o SDK do site, sem problemas بعد تنفيذ تعليمة الحرق.

## O SDK está disponível

- [Atualizado para Rust SDK](/sdks/rust)
- [Implementar para Python SDK](/sdks/python)
- [Escolha o JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/nft-flow.ko)

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