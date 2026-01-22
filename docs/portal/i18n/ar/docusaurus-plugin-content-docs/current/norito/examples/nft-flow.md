<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/nft-flow
title: سك ونقل وحرق NFT
description: يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.

## جولة دفتر الأستاذ

- تأكد من وجود تعريف NFT (مثل `n0#wonderland`) إلى جانب حسابات المالك/المستلم المستخدمة في المقتطف (`ih58...`, `ih58...`).
- استدعِ نقطة الدخول `nft_issue_and_transfer` لسك NFT ونقله من Alice إلى Bob وإرفاق علامة بيانات تعريف تصف الإصدار.
- افحص حالة دفتر NFT باستخدام `iroha_cli nfts list --account <id>` أو مكافئات SDK للتحقق من النقل، ثم أكد إزالة الأصل بعد تنفيذ تعليمة الحرق.

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
