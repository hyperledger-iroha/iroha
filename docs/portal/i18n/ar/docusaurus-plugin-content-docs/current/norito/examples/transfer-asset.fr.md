---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/transfer-asset
العنوان: نقل نشاط بين الحسابات
الوصف: تدفق نقل الأنشطة البسيط الذي يعكس Quickstarts SDK وساحات التسجيل.
المصدر: أمثلة/نقل/transfer.ko
---

تدفق نقل الأنشطة البسيط الذي يعكس Quickstarts SDK وساحات التسجيل.

## باركور دو ريجيستري

- قم بتمويل Alice باستخدام النشاط التجاري (على سبيل المثال عبر المقتطف `register and mint` أو تدفق Quickstart SDK).
- قم بتنفيذ نقطة الدخول `do_transfer` لتحريك 10 وحدات من Alice إلى Bob، مع الحصول على الإذن `AssetTransferRole`.
- قم بالتحقق من العناصر (`FindAccountAssets`, `iroha_cli ledger assets list`) أو قم بمتابعة أحداث خط الأنابيب لمراقبة نتيجة النقل.

## أدلة شركاء SDK

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```