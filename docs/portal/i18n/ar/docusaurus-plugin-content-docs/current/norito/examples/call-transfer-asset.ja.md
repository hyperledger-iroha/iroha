---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a9b6550f88d87502f1628f6310bd582ffd411efde921165d3f5f5ec26e28e9d8
source_last_modified: "2026-01-22T15:55:01+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/call-transfer-asset
title: استدعاء نقل المضيف من Kotodama
description: يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.

## جولة دفتر الأستاذ

- موّل سلطة العقد (مثلا `<katakana-i105-account-id>`) بالأصل الذي ستنقله وامنح السلطة دور `CanTransfer` أو إذنا مكافئا.
- استدعِ نقطة الدخول `call_transfer_asset` لنقل 5 وحدات من حساب العقد إلى `<katakana-i105-account-id>`، بما يعكس طريقة تغليف الأتمتة على السلسلة لنداءات المضيف.
- تحقق من الأرصدة عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account <katakana-i105-account-id>` وافحص الأحداث لتأكيد أن حارس بيانات التعريف سجل سياق النقل.

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
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
