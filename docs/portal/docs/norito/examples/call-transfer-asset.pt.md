<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a91fc8841580a836c80129942df7f79f5bc5dd5f6a72dccf1394b740d02536a5
source_last_modified: "2025-11-23T15:30:33.687233+00:00"
translation_last_reviewed: 2025-12-30
---

---
slug: /norito/examples/call-transfer-asset
title: Invocar transferencia do host a partir de Kotodama
description: Mostra como um entrypoint Kotodama pode chamar a instrucao do host `transfer_asset` com validacao inline de metadados.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Mostra como um entrypoint Kotodama pode chamar a instrucao do host `transfer_asset` com validacao inline de metadados.

## Roteiro do livro razao

- Financie a autoridade do contrato (por exemplo `ih58...`) com o ativo que ela transferira e conceda a autoridade o papel `CanTransfer` ou permissao equivalente.
- Chame o entrypoint `call_transfer_asset` para transferir 5 unidades da conta do contrato para `ih58...`, refletindo como a automacao on-chain pode envolver chamadas do host.
- Verifique saldos via `FindAccountAssets` ou `iroha_cli assets list --account ih58...` e inspecione eventos para confirmar que o guard de metadados registrou o contexto da transferencia.

## Guias de SDK relacionados

- [Quickstart do SDK Rust](/sdks/rust)
- [Quickstart do SDK Python](/sdks/python)
- [Quickstart do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
