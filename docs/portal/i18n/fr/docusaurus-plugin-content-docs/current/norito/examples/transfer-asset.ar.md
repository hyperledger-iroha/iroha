---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/transfer-asset
titre : نقل أصل بين الحسابات
description : Vous trouverez ici le SDK et le SDK.
source : exemples/transfert/transfer.ko
---

Vous devez utiliser le SDK pour installer le SDK.

## جولة دفتر الأستاذ

- موّل Alice بالأصل المستهدف مسبقا (على سبيل المثال عبر المقتطف `register and mint` et تدفقات البدء السريع للـ SDK).
- نفّذ نقطة الدخول `do_transfer` لنقل 10 ans avec Alice et Bob مع استيفاء إذن `AssetTransferRole`.
- استعلم أرصدة (`FindAccountAssets`, `iroha_cli ledger assets list`) et اشترك في أحداث خط الأنابيب لملاحظة نتيجة النقل.

## Le SDK est disponible

- [Détails du SDK Rust](/sdks/rust)
- [Détails du SDK Python](/sdks/python)
- [Détails du SDK JavaScript](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```