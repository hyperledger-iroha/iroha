---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/transfer-asset
العنوان: انتقال الأثاث الجلدي
الوصف: أدوات بسيطة لإدارة السفر والبدء السريع لـ SDK والإرشادات التفصيلية البسيطة للكلمات الرئيسية.
المصدر: أمثلة/نقل/transfer.ko
---

الأدوات البسيطة التي تعمل على السفر هي البداية السريعة لـ SDK والإرشادات التفصيلية الصحيحة.

## ليجر واک تھرو

- أليس لديها أثاث جديد لتكنولوجيا المعلومات (مثل `register and mint` أو برنامج SDK Quickstart فلو).
- انتقلت `do_transfer` عبر الإنترنت إلى أليس وبوب حيث انتقلت 10 يونا، وتم السماح بـ `AssetTransferRole`.
- بيلنس (`FindAccountAssets`, `iroha_cli ledger assets list`) يتم إجراء عملية تحويل قروض صغيرة أو ورق عبر الإنترنت.

## مواضيع ذات صلة SDK

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Kotodama تنزيل التنزيل](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```