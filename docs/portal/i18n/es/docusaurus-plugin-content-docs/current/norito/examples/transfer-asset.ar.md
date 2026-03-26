---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/ejemplos/transfer-asset
título: نقل أصل بين الحسابات
descripción: سير عمل بسيط لنقل الأصول يعكس بدايات SDK السريعة وجولات دفتر الأستاذ.
fuente: ejemplos/transfer/transfer.ko
---

Para obtener más información, consulte el SDK y los archivos adjuntos.

## جولة دفتر الأستاذ

- Mi nombre es Alice (hay un archivo `register and mint` que contiene el SDK).
- نفّذ نقطة الدخول `do_transfer` لنقل 10 وحدات من Alice إلى Bob مع استيفاء إذن `AssetTransferRole`.
- استعلم عن الأرصدة (`FindAccountAssets`, `iroha_cli ledger assets list`) أو اشترك في أحداث خط الأنابيب لملاحظة نتيجة النقل.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [Aplicación del SDK de Python](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Actualización Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```