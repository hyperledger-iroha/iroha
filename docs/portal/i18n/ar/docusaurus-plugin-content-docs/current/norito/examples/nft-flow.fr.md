---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/nft-flow
العنوان: Frapper، نقل وتسخين NFT
الوصف: دورة حياة NFT من نوبة في نوبة: قطع الملكية ونقلها وإضافة الاستبدالات والتدمير.
المصدر: صناديق/ivm/docs/examples/12_nft_flow.ko
---

أكمل دورة حياة NFT في نوبة: إنهاء الملكية والنقل وإضافة المخلفات والتدمير.

## باركور دو ريجيستري

- تأكد من أن تعريف NFT (على سبيل المثال `n0#wonderland`) موجود مع الحسابات الشخصية/الوجهة المستخدمة في المقتطف (`soraカタカナ...`، `soraカタカナ...`).
- قم باستدعاء نقطة الدخول `nft_issue_and_transfer` لضبط NFT ونقل Alice إلى Bob وإرفاق مؤشر ميتادونييز صادر عن الإصدار.
- افحص حالة تسجيل NFT مع `iroha_cli ledger nfts list --account <id>` أو SDK المكافئة للتحقق من النقل، ثم تأكد من أن النشاط قد تم حذفه مرة واحدة حتى يتم تنفيذ تعليمات النسخ.

## أدلة شركاء SDK

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/nft-flow.ko)

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