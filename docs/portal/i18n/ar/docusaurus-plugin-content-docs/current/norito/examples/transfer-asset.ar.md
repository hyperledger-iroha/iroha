---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/transfer-asset
العنوان: نقل أصلي بين ضباط
الوصف: سير عمل بسيط لحفظ الأدلة وبدايات SDK السريعة وجولات أستاذ الأستاذ.
المصدر: أمثلة/نقل/transfer.ko
---

سير العمل الرئيسي لسجلات السجلات ودلائل SDK السريعة وجولات دفتر الأستاذ.

## جولة أستاذ الأستاذ

- موّل Alice بالأصل المستهدف المسبق (على سبيل المثال، عبر المقتطف `register and mint` أو تدفقات المراقبة السريعة لـ SDK).
- نفيذ نقطة الدخول `do_transfer` نقل 10 وحدات من Alice إلى Bob مع استيفاء إذن `AssetTransferRole`.
- استعلم عن الأرصدة (`FindAccountAssets`, `iroha_cli ledger assets list`) أو اشترك في أحداث خط الأنابيب لملاحظة نتيجة النقل.

## دليل SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
      account!("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10,
      dataspace_id("0")
    );
  }
}
```