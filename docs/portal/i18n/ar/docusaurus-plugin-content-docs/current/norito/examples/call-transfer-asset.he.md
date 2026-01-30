---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17d023a1163f973d1cbcb72f4031e28aabfcbe0ca0866c1e1d4cc26f543c7ce8
source_last_modified: "2025-11-14T04:43:20.646179+00:00"
translation_last_reviewed: 2026-01-30
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
