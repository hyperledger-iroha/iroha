---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Invocar transferência de host a partir de Kotodama
description: Mostra como um ponto de entrada Kotodama pode chamar a instrução do host `transfer_asset` com validação inline de metadados.
fonte: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Mostrando como um ponto de entrada Kotodama pode chamar a instrução do host `transfer_asset` com validação inline de metadados.

## Roteiro do livro razão

- Financiar a autoridade do contrato (por exemplo `i105...`) com o ativo que ela transfere e conceder a autoridade o papel `CanTransfer` ou permissão equivalente.
- Chame o ponto de entrada `call_transfer_asset` para transferir 5 unidades da conta do contrato para `i105...`, refletindo como a automação on-chain pode envolver chamadas do host.
- Verifique saldos via `FindAccountAssets` ou `iroha_cli ledger assets list --account i105...` e inspecione eventos para confirmar que o guarda de metadados registrado no contexto da transferência.

## Guias de SDK relacionados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [Início rápido do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```