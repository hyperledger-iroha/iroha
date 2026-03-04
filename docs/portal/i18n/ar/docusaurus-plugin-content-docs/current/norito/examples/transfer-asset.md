---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/transfer-asset
title: نقل أصل بين الحسابات
description: سير عمل بسيط لنقل الأصول يعكس بدايات SDK السريعة وجولات دفتر الأستاذ.
source: examples/transfer/transfer.ko
---

سير عمل بسيط لنقل الأصول يعكس بدايات SDK السريعة وجولات دفتر الأستاذ.

## جولة دفتر الأستاذ

- موّل Alice بالأصل المستهدف مسبقا (على سبيل المثال عبر المقتطف `register and mint` أو تدفقات البدء السريع للـ SDK).
- نفّذ نقطة الدخول `do_transfer` لنقل 10 وحدات من Alice إلى Bob مع استيفاء إذن `AssetTransferRole`.
- استعلم عن الأرصدة (`FindAccountAssets`, `iroha_cli ledger assets list`) أو اشترك في أحداث خط الأنابيب لملاحظة نتيجة النقل.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/transfer-asset.ko)

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
