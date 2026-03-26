---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラッグ: /norito/examples/call-transfer-asset
タイトル: استدعاء نقل المضيف من Kotodama
説明: يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن重要です。
ソース: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن منやあ。

## جولة دفتر الأستاذ

- موّل سلطة العقد (مثلا `soraカタカナ...`) بالأصل الذي ستنقله وامنح السلطة دور `CanTransfer` أو إذناああ。
- `call_transfer_asset` 5 وحساب العقد إلى `soraカタカナ...` بما يعكس最高のパフォーマンスを見せてください。
- 評価 `FindAccountAssets` أو `iroha_cli ledger assets list --account soraカタカナ...` وافحص الأحداث لتأكيد أن حارس بياناتありがとうございます。

## SDK の開発

- [Rust SDK](/sdks/rust)
- [Python SDK](/sdks/python)
- [JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```