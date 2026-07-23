---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/nft-flow
العنوان: شراء وتحويل ومتابعة NFT
الوصف: توفير دورة حياة NFT من البداية إلى النهاية: إعادة الشحن والتحويل والتحسين والنقل.
المصدر: صناديق/ivm/docs/examples/12_nft_flow.ko
---

يوفر دورة حياة NFT من البداية إلى النهاية: إعادة الشحن والتحويل والتحسين والصيانة.

## Почаговый обдод еестра

- تأكد من أن بروتوكول NFT (على سبيل المثال `n0#wonderland`) موجود مباشرة مع حساب الخادم/المستخدم، مستخدمًا في المقتطف (`<i105-account-id>`، `<i105-account-id>`).
- قم باختيار `nft_issue_and_transfer` من أجل استخدام NFT وتحويل نفسه من Alice إلى Bob وكسر علم الميتادانات، الوصف.
- التحقق من إعدادات NFT-rest عبر `iroha_cli ledger nfts list --account <id>` أو ما يعادلها من SDK، من أجل إعادة التحديث ثم الانضمام إلى ما هو نشط يتم استكماله بعد تعليمات الاستخدام للحرق.

## تطوير شامل SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[تحميل النسخة Kotodama](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", );
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76", );
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
