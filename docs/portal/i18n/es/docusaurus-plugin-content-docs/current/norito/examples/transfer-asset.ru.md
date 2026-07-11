---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/ejemplos/transfer-asset
título: Перевести актив между аккаунтами
descripción: Establecimiento de escenarios activos, SDK de inicio rápido y registro de tutoriales.
fuente: ejemplos/transfer/transfer.ko
---

Hay escenarios previos activos, un SDK de inicio rápido y un registro de tutoriales.

## Пошаговый обход реестра

- La popular aplicación Alice (por ejemplo, el fragmento `register and mint` o el SDK de inicio rápido).
- Utilice este vídeo `do_transfer`, ya lleva 10 ediciones de Alice y Bob, siguiendo el ejemplo `AssetTransferRole`.
- Pruebe los saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) o conecte la tubería de sobытия, para que pueda obtener el resultado deseado.

## Связанные руководства SDK

- [SDK de inicio rápido de Rust](/sdks/rust)
- [SDK de Python de inicio rápido](/sdks/python)
- [SDK de JavaScript de inicio rápido](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/transfer-asset.ko)

```kotodama
// Transfer example: uses typed pointer constructors and transfer_asset syscall
seiyaku TransferDemo {
    // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
    kotoage fn do_transfer() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse(
                "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            ),
            destination: AccountId::parse(
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: 10,
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
