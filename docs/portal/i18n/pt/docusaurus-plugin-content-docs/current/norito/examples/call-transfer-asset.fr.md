---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Invoquer le transfert hôte depuis Kotodama
descrição: Montar comentário no ponto de entrada Kotodama pode chamar a instrução do hotel `transfer_asset` com validação de metadonées on-line.
fonte: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Se você comentar um ponto de entrada Kotodama, poderá acessar a instrução `transfer_asset` com validação de metadonées on-line.

## Parcours du registre

- Aprovar a autoridade do contrato (por exemplo `<i105-account-id>`) com o ato de transferência e conceder a função `CanTransfer` ou uma permissão equivalente.
- Ligue para o ponto de entrada `call_transfer_asset` para transferir 5 unidades da conta do contrato para `<i105-account-id>`, de modo que a maneira como a automatização na cadeia não pode encapsular as chamadas em casa.
- Verifique as vendas via `FindAccountAssets` ou `iroha_cli ledger assets list --account <i105-account-id>` e inspecione os eventos para confirmar que o acúmulo de metadonos foi publicado no contexto da transferência.

## Guias SDK associados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [Início rápido do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/call-transfer-asset.ko)

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