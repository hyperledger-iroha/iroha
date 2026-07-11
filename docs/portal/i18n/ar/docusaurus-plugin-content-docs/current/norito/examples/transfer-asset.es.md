---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/transfer-asset
العنوان: نقل النشاط بين الحسابات
الوصف: تدفق مباشر لنقل الأنشطة التي تعكس البدايات السريعة لـ SDK والمسجلات الأكبر للكتاب.
المصدر: أمثلة/نقل/transfer.ko
---

يتدفق مباشرة نقل الأنشطة التي تعكس البدايات السريعة لـ SDK والمسجلات الأكبر للكتاب.

## Recorrido del libro mayor

- قم بالبدء مسبقًا في Alice باستخدام الهدف النشط (على سبيل المثال عبر الجزء `register and mint` أو تدفقات التشغيل السريع من SDK).
- قم بتشغيل نقطة الدخول `do_transfer` لتحريك 10 وحدات من Alice إلى Bob، بعد الحصول على الإذن `AssetTransferRole`.
- راجع الأرصدة (`FindAccountAssets`، `iroha_cli ledger assets list`) أو اشترك في أحداث خط الأنابيب لمراقبة نتيجة النقل.

## أدلة SDK ذات الصلة

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK لـ JavaScript](/sdks/javascript)

[تنزيل مصدر Kotodama](/norito-snippets/transfer-asset.ko)

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
