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

- إحدى أدوات NFT (مثل `n0#wonderland`) الموجودة والمتاحة للاستخدام مرة واحدة وملكية الوصول/الوصول (`<i105-account-id>`, `<i105-account-id>`) موجود بالفعل.
- `nft_issue_and_transfer` عبر تقنية NFT للعبة NFT، تنتقل Alice S Bob إلى الأمام، وتفتح نافذة جديدة ومتجددة الهواء من منسلك.
- `iroha_cli ledger nfts list --account <id>` أو SDK هو استخدام بديل لقاعدة NFT الخاصة بالسجلات، مما يسمح لك بتمرير البحث، ثم مسح التطبيق بعد حذفه جاتا ہے۔

## مواضيع ذات صلة SDK

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Kotodama تنزيل التنزيل](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse(
            "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
        );
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse(
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
        );
        ledger::nft::transfer(source: owner, nft: nft, destination: to);
        ledger::nft::set_metadata(
            nft: nft,
            key: Name::parse("issued"),
            value: Json::parse("{\"issued\":\"demo\"}"),
        );
        ledger::nft::burn(nft);
    }
}
```
