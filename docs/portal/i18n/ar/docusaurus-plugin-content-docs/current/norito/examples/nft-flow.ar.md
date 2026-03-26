---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/nft-flow
العنوان: سك المتوقع وحرق NFT
الوصف: يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.
المصدر: صناديق/ivm/docs/examples/12_nft_flow.ko
---

يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.

## جولة أستاذ الأستاذ

- تأكد من وجود تعريف NFT (مثل `n0#wonderland`) إلى جانب الحسابات التجارية/المستعملة المستلمة في المقتطف (`i105...`, `i105...`).
- المؤكد نقطة الدخول `nft_issue_and_transfer` لسك NFT متوقعه من Alice إلى Bob ويرافق علامة تعريف بيانات تعريف تصف الإصدار.
- فحص حالة الكمبيوتر NFT باستخدام `iroha_cli ledger nfts list --account <id>` أو مكافئات SDK مؤكدًا على التغيير، ثم شدد على إزالة الأصل بعد تنفيذ تعليمة الحرق.

## دليل SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("soraカタカナ...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("soraカタカナ...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```