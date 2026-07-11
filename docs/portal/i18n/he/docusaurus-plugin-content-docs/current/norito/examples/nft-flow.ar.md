---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
title: سك ونقل وحرق NFT
description: يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.
מקור: crates/ivm/docs/examples/12_nft_flow.ko
---

يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف، والحرق.

## جولة دفتر الأستاذ

- تأكد من وجود تعريف NFT (مثل `n0#wonderland`) إلى جانب حسابات المالك/المستلم المستخدمة في المقتطف (`<i105-account-id>`, `<i105-account-id>`).
- استدعِ نقطة الدخول `nft_issue_and_transfer` لسك NFT ونقله من Alice إلى Bob وإرفاق علامة بيانات تعريف تصف الإصدار.
- התקנת NFT תקנת `iroha_cli ledger nfts list --account <id>` ו-SDK של תקליטורים. بعد تنفيذ تعليمة الحرق.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/nft-flow.ko)

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
