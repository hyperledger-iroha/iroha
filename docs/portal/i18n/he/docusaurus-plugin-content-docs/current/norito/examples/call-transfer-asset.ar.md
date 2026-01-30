---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
slug: /norito/examples/call-transfer-asset
title: استدعاء نقل المضيف من Kotodama
description: يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.

## جولة دفتر الأستاذ

- موّل سلطة العقد (مثلا `ih58...`) بالأصل الذي ستنقله وامنح السلطة دور `CanTransfer` أو إذنا مكافئا.
- استدعِ نقطة الدخول `call_transfer_asset` لنقل 5 وحدات من حساب العقد إلى `ih58...`، بما يعكس طريقة تغليف الأتمتة على السلسلة لنداءات المضيف.
- تحقق من الأرصدة عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account ih58...` وافحص الأحداث لتأكيد أن حارس بيانات التعريف سجل سياق النقل.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
