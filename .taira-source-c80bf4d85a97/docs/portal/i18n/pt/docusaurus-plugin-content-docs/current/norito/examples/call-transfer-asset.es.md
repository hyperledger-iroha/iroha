---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Invocar transferência de host de Kotodama
description: Mostrar como um ponto de entrada de Kotodama pode ser chamado de instrução de host `transfer_asset` com validação de metadados on-line.
fonte: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Mostrar como um ponto de entrada de Kotodama pode ser chamado de instrução do host `transfer_asset` com validação de metadados on-line.

## Recorrido do livro prefeito

- Funda a autoridade do contrato (por exemplo `<i105-account-id>`) com o ativo que será transferido e otórgale o papel `CanTransfer` ou uma permissão equivalente.
- Ligue para o ponto de entrada `call_transfer_asset` para transferir 5 unidades da conta do contrato para `<i105-account-id>`, refletindo a forma em que a automatização na cadeia pode envolver chamadas do host.
- Verifique os saldos através de `FindAccountAssets` ou `iroha_cli ledger assets list --account <i105-account-id>` e inspecione os eventos para confirmar que a guarda de metadados registrou o contexto da transferência.

## Guias do SDK relacionados

- [Início rápido do SDK de Rust](/sdks/rust)
- [Início rápido do SDK de Python](/sdks/python)
- [Início rápido do SDK de JavaScript](/sdks/javascript)

[Descarregue a fonte de Kotodama](/norito-snippets/call-transfer-asset.ko)

```kotodama
// Direct builtin call (no raw call syntax) inside a seiyaku.
seiyaku TransferCall {
    kotoage fn pay() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", ),
            destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76", ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: 10,
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
