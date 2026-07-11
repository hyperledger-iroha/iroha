---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/transfer-asset
título: Transferir um ato entre contas
description: Fluxo de transferência de atividades simples que reflete o SDK de início rápido e as etapas de registro.
fonte: exemplos/transfer/transfer.ko
---

Fluxo de transferência de atividades simples que reflete o SDK de início rápido e os pacotes de registro.

## Parcours du registre

- Pré-financiar Alice com o ativo (por exemplo, por meio do snippet `register and mint` ou do fluxo do SDK de início rápido).
- Execute o ponto de entrada `do_transfer` para substituir 10 unidades de Alice de Bob, satisfazendo a permissão `AssetTransferRole`.
- Interrogue as soldas (`FindAccountAssets`, `iroha_cli ledger assets list`) ou conecte-se a eventos do pipeline para observar o resultado da transferência.

## Guias SDK associados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [Início rápido do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/transfer-asset.ko)

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
