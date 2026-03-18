---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/nft-flow
العنوان: NFT هو من، ينتقل ويسجل
description: NFT هو عبارة عن بطاقة ابتكارية رائعة: ملكية كبيرة، تنتقل إلى ملكية جديدة، وبرنامج للملكية.
المصدر: صناديق/ivm/docs/examples/12_nft_flow.ko
---

NFT هي عبارة عن ابتكار جديد من نوعه: ملكية كبيرة، انتقال ملكية، المزيد من المزايا، والتطبيق.

## ليجر واک تھرو

- إحدى أدوات NFT (مثل `n0#wonderland`) الموجودة والمتاحة للاستخدام مرة واحدة وملكية الوصول/الوصول (`i105...`, `i105...`) موجود بالفعل.
- `nft_issue_and_transfer` عبر تقنية NFT للعبة NFT، تنتقل Alice S Bob إلى الأمام، وتفتح نافذة جديدة ومتجددة الهواء من منسلك.
- `iroha_cli ledger nfts list --account <id>` أو SDK هو استخدام بديل لقاعدة NFT الخاصة بالسجلات، مما يسمح لك بتمرير البحث، ثم مسح التطبيق بعد حذفه جاتا ہے۔

## مواضيع ذات صلة SDK

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Kotodama تنزيل التنزيل](/norito-snippets/nft-flow.ko)

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