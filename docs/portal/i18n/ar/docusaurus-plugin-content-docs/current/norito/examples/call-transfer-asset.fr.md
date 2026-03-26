---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/call-transfer-asset
العنوان: Invoquer le Transfert hôte depuis Kotodama
description: Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.
المصدر: صناديق/ivm/docs/examples/08_call_transfer_asset.ko
---

قم بالتعليق على نقطة دخول Kotodama ويمكن استدعاء التعليمات الساخنة `transfer_asset` مع التحقق من صحة البيانات عبر الإنترنت.

## باركور دو ريجيستري

- الموافقة على صلاحية العقد (على سبيل المثال `<i105-account-id>`) مع النشاط الذي يتم نقله ومنحه الدور `CanTransfer` أو إذن مكافئ.
- Appelez le point d'entrée `call_transfer_asset` لنقل 5 وحدات من حساب العقد إلى `<i105-account-id>`، مما يعكس طريقة الأتمتة على السلسلة والتي قد تؤدي إلى تغليف المكالمات الساخنة.
- تحقق من العناصر عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account <i105-account-id>` وافحص الأحداث للتأكد من أن حارس البيانات يتم تسجيله في سياق النقل.

## أدلة شركاء SDK

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```