---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/transfer-asset
title: Transferir ativo entre contas
description: Fluxo direto de transferencia de ativos que espelha os quickstarts do SDK e os roteiros do livro razao.
source: examples/transfer/transfer.ko
---

Fluxo direto de transferencia de ativos que espelha os quickstarts do SDK e os roteiros do livro razao.

## Roteiro do livro razao

- Pre-financie Alice com o ativo alvo (por exemplo via o trecho `register and mint` ou os fluxos de quickstart do SDK).
- Execute o entrypoint `do_transfer` para mover 10 unidades de Alice para Bob, atendendo a permissao `AssetTransferRole`.
- Consulte saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) ou assine eventos do pipeline para observar o resultado da transferencia.

## Guias de SDK relacionados

- [Quickstart do SDK Rust](/sdks/rust)
- [Quickstart do SDK Python](/sdks/python)
- [Quickstart do SDK JavaScript](/sdks/javascript)

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
