---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/nft-flow
العنوان: Cunhar, Transferir e queimar um NFT
الوصف: قم بإعادة دورة حياة NFT في بداية الفيلم: جمع التبرعات، والتحويلات، وتسجيل الميتادادوس، وما إلى ذلك.
المصدر: صناديق/ivm/docs/examples/12_nft_flow.ko
---

قم باستعادة دورة حياة NFT في بداية الفيلم: جمع التبرعات، والتحويلات، وتوقيع ميتادادوس، وما إلى ذلك.

## Roteiro do livro razao

- ضمان أن تعريف NFT (على سبيل المثال `n0#wonderland`) موجود جنبًا إلى جنب مع حسابات التبرع/الوجهة المستخدمة (`<i105-account-id>`، `<i105-account-id>`).
- قم باستدعاء نقطة الدخول `nft_issue_and_transfer` لإنشاء NFT، ونقل Alice إلى Bob، وإدخال مجموعة من البيانات التي تكتشفها.
- فحص حالة كتاب NFT مع `iroha_cli ledger nfts list --account <id>` أو ما يعادلها من SDK للتحقق من النقل، بعد التأكد من القيام بالمهمة وإزالتها عند تلقي تعليمات هذا الأمر.

## أدلة SDK ذات الصلة

- [بدء التشغيل السريع لـ SDK Rust](/sdks/rust)
- [البدء السريع لـ SDK Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK JavaScript](/sdks/javascript)

[اضغط على الخط Kotodama](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76");
        ledger::nft::transfer(owner, nft, to);
        ledger::nft::set_metadata(nft, Name::parse("issued"), Json::parse("{\"issued\":\"demo\"}"));
        ledger::nft::burn(nft);
    }
}
```
