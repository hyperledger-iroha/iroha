---
lang: pt
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
description: يوضح كيف يمكن لنقطة دخول Kotodama مع التحقق المضمن من بيانات التعريف.
fonte: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Você pode usar o Kotodama para obter mais informações sobre `transfer_asset` بيانات التعريف.

## جولة دفتر الأستاذ

- O código de barras (`<i105-account-id>`) está disponível para uso em `CanTransfer` ou `CanTransfer`. Então.
- Use o `call_transfer_asset` para 5 e coloque-o no `<i105-account-id>`. Verifique se o produto está funcionando corretamente.
- Você pode usar `FindAccountAssets` ou `iroha_cli ledger assets list --account <i105-account-id>` para obter mais informações. التعريف سجل سياق النقل.

## O SDK está disponível

- [Desenvolver o Rust SDK](/sdks/rust)
- [Implementar para Python SDK](/sdks/python)
- [Escolha o JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/call-transfer-asset.ko)

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
