---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/transfer-asset
العنوان: Transferir ativo entre contas
الوصف: التدفق المباشر لنقل المهام الذي يستكشف البدايات السريعة لـ SDK والنسخ الاحتياطية للكتاب.
المصدر: أمثلة/نقل/transfer.ko
---

التدفق المباشر لنقل المهام الذي يستكشف البدايات السريعة لـ SDK والنسخ الاحتياطية للكتاب.

## Roteiro do livro razao

- التمويل المسبق لـ Alice مع مهمة أخرى (على سبيل المثال عبر `register and mint` أو تدفقات التشغيل السريع لـ SDK).
- قم بتنفيذ نقطة الدخول `do_transfer` لتحريك 10 وحدات من Alice لـ Bob، ثم استمع إلى السماح `AssetTransferRole`.
- راجع البيانات (`FindAccountAssets`، `iroha_cli ledger assets list`) أو قم بإجراء الأحداث لمراقبة نتيجة النقل.

## أدلة SDK ذات الصلة

- [بدء التشغيل السريع لـ SDK Rust](/sdks/rust)
- [البدء السريع لـ SDK Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK JavaScript](/sdks/javascript)

[اضغط على الخط Kotodama](/norito-snippets/transfer-asset.ko)

```kotodama
// Transfer example: uses typed pointer constructors and transfer_asset syscall
seiyaku TransferDemo {
    // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
    kotoage fn do_transfer() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse(
                "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            ),
            destination: AccountId::parse(
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: Amount::from_i64(10),
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
