---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c2168c60ef54f86070fc49da38d48ff1ebf18f37e16e26e34cf4d9856cd524e8
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


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
