---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 21f5bd15fbe8d24bb4d4e8297a2bfdd34dd301a0061d0f86a27e28ddf4c2538b
source_last_modified: "2025-11-14T04:43:20.772448+00:00"
translation_last_reviewed: 2026-01-30
---

يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.

## جولة دفتر الأستاذ

- تأكد من وجود تعريف NFT (مثل `n0#wonderland`) إلى جانب حسابات المالك/المستلم المستخدمة في المقتطف (`ih58...`, `ih58...`).
- استدعِ نقطة الدخول `nft_issue_and_transfer` لسك NFT ونقله من Alice إلى Bob وإرفاق علامة بيانات تعريف تصف الإصدار.
- افحص حالة دفتر NFT باستخدام `iroha_cli ledger nfts list --account <id>` أو مكافئات SDK للتحقق من النقل، ثم أكد إزالة الأصل بعد تنفيذ تعليمة الحرق.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/nft-flow.ko)

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
