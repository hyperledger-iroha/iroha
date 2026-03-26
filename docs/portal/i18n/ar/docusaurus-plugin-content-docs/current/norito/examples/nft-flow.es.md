---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/nft-flow
العنوان: Acuñar, Transferir y quemar un NFT
الوصف: قم بإعادة دورة الحياة من NFT من أقصى إلى أقصى: الضغط على الملكية، والتحويل، وبيانات التعريف، والمفتاح.
المصدر: صناديق/ivm/docs/examples/12_nft_flow.ko
---

قم باستعادة دورة الحياة من NFT من أقصى إلى أقصى: الضغط على الملكية، والتحويل، وبيانات التعريف، والمفتاح.

## Recorrido del libro mayor

- تأكد من وجود تعريف NFT (على سبيل المثال `n0#wonderland`) جنبًا إلى جنب مع حسابات المالك/المستقبل المستخدمة في الجزء (`<i105-account-id>`، `<i105-account-id>`).
- استدعاء نقطة الدخول `nft_issue_and_transfer` لالتقاط NFT ونقل Alice إلى Bob وإضافة شريط بيانات يصف الإصدار.
- فحص حالة NFT الأكبر مع `iroha_cli ledger nfts list --account <id>` أو SDK المكافئة للتحقق من النقل، حيث يؤكد أن النشاط سيتم حذفه بمجرد تنفيذ تعليمات الانتظار.

## أدلة SDK ذات الصلة

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK لـ JavaScript](/sdks/javascript)

[تنزيل مصدر Kotodama](/norito-snippets/nft-flow.ko)

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