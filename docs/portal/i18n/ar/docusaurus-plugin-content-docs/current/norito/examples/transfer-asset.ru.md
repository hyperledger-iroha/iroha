---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/transfer-asset
العنوان: التحويل النشط بين الحسابات
الوصف: سيناريو أولي ينقل الأنشطة، وSDK خارجي لـ Quickstart، ومسجل إرشادات تفصيلية.
المصدر: أمثلة/نقل/transfer.ko
---

سيناريو أولي ينقل الأنشطة، بالإضافة إلى مجموعة أدوات SDK الخاصة بـ Quickstart ومسجل الإرشادات التفصيلية.

## Почаговый обдод еестра

- قم بملء نشاط Alice الخلوي مسبقًا (على سبيل المثال من خلال المقتطف `register and mint` أو Quickstart SDK).
- المياه الصالحة للشرب `do_transfer` لتحويل 10 أجزاء من Alice إلى Bob، والتمتع بالرضا عن `AssetTransferRole`.
- تحقق من التوازن (`FindAccountAssets`، `iroha_cli ledger assets list`) أو قم بإدخال خط أنابيب الاشتراك لرؤية نتيجة التحويل.

## تطوير شامل SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[تحميل النسخة Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```