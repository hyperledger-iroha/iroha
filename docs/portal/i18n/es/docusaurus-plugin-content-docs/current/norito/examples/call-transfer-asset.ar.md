---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: استدعاء نقل المضيف من Kotodama
descripción: يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.
fuente: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة الضيف `transfer_asset` مع التحقق المضمن من بيانات التعريف.

## جولة دفتر الأستاذ

- Utilice el cable de alimentación (`<i105-account-id>`) para conectar el cable y el conector de `CanTransfer` y el cable de alimentación.
- Presione el botón `call_transfer_asset` para 5 veces el conector `<i105-account-id>`, y luego presione el botón `<i105-account-id>`. الأتمتة على السلسلة لنداءات المضيف.
- Para cambiar la configuración de `FindAccountAssets` o `iroha_cli ledger assets list --account <i105-account-id>` y de las opciones de configuración سياق النقل.

## أدلة SDK ذات صلة

- [Aplicación del SDK de Rust](/sdks/rust)
- [Aplicación del SDK de Python](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Actualización Kotodama](/norito-snippets/call-transfer-asset.ko)

```kotodama
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
    kotoage fn pay() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse(
                "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            ),
            destination: AccountId::parse(
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: Amount::from_i64(10),
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
