---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/call-transfer-asset
titre : استدعاء نقل المضيف من Kotodama
description: يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.
source : crates/ivm/docs/examples/08_call_transfer_asset.ko
---

يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.

## جولة دفتر الأستاذ

- موّل سلطة العقد (مثلا `i105...`) pour أصل الذي ستنقله وامنح السلطة دور `CanTransfer` أو إذنا مكافئا.
- استدعِ نقطة الدخول `call_transfer_asset` لنقل 5 وحدات من حساب العقد إلى `i105...`, بما يعكس طريقة تغليف الأتمتة على السلسلة لنداءات المضيف.
- تحقق من أرصدة عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account i105...` وافحص الأحداث لتأكيد أن حارس بيانات التعريف سجل سياق النقل.

## Le SDK est disponible

- [Détails du SDK Rust](/sdks/rust)
- [Détails du SDK Python](/sdks/python)
- [Détails du SDK JavaScript](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```